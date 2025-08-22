use crate::config::{ClientConfig, RetryStrategy};
use crate::connection::{ConnectionPool, AgentConnection, ConnectionState};
use crate::error::{ClientError, ClientResult};
use remotefs_common::protocol::{
    Message, FileMetadata, DirEntry, generate_request_id
};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use tracing::{debug, info, warn};
use bytes::Bytes;

/// Main RemoteFS client
pub struct RemoteFsClient {
    /// Client configuration
    config: ClientConfig,
    
    /// Connection pool for managing agent connections
    connection_pool: ConnectionPool,
    
    /// Client statistics
    stats: Arc<RwLock<ClientStats>>,
}

/// Client statistics
#[derive(Debug, Clone, Default)]
pub struct ClientStats {
    pub operations_total: u64,
    pub operations_successful: u64,
    pub operations_failed: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub avg_response_time_ms: f64,
    pub active_connections: u32,
}

impl RemoteFsClient {
    /// Create a new RemoteFS client
    pub fn new(config: ClientConfig) -> ClientResult<Self> {
        config.validate()?;
        
        let connection_pool = ConnectionPool::new(config.connection.clone());
        
        let client = Self {
            config,
            connection_pool,
            stats: Arc::new(RwLock::new(ClientStats::default())),
        };
        
        Ok(client)
    }
    
    /// Initialize the client and connect to agents
    pub async fn initialize(&self) -> ClientResult<()> {
        info!("Initializing RemoteFS client with {} agents", self.config.agents.len());
        
        // Add all enabled agents to the connection pool
        for agent_config in self.config.enabled_agents() {
            self.connection_pool.add_agent(agent_config.clone()).await;
        }
        
        // Connect to all agents
        let results = self.connection_pool.connect_all().await;
        let mut successful_connections = 0;
        let mut failed_connections = 0;
        
        for (i, result) in results.iter().enumerate() {
            match result {
                Ok(()) => {
                    successful_connections += 1;
                    info!("Successfully connected to agent {}", self.config.agents[i].id);
                }
                Err(e) => {
                    failed_connections += 1;
                    warn!("Failed to connect to agent {}: {}", self.config.agents[i].id, e);
                }
            }
        }
        
        if successful_connections == 0 {
            return Err(ClientError::Connection(
                "Failed to connect to any agents".to_string()
            ));
        }
        
        info!(
            "Client initialized: {} successful connections, {} failed connections",
            successful_connections, failed_connections
        );
        
        Ok(())
    }
    
    /// Shutdown the client and disconnect from all agents
    pub async fn shutdown(&self) -> ClientResult<()> {
        info!("Shutting down RemoteFS client");
        
        let results = self.connection_pool.disconnect_all().await;
        for result in results {
            if let Err(e) = result {
                warn!("Error during shutdown: {}", e);
            }
        }
        
        info!("RemoteFS client shutdown complete");
        Ok(())
    }
    
    /// Read a file from the remote filesystem
    pub async fn read_file<P: AsRef<Path>>(&self, path: P) -> ClientResult<Bytes> {
        self.read_file_range(path, None, None).await
    }
    
    /// Read a file range from the remote filesystem
    pub async fn read_file_range<P: AsRef<Path>>(
        &self,
        path: P,
        offset: Option<u64>,
        length: Option<u64>,
    ) -> ClientResult<Bytes> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        
        let request = Message::ReadFile {
            request_id: generate_request_id(),
            path: path_str.clone(),
            offset: offset.unwrap_or(0),
            length: length.map(|l| l as u32).unwrap_or(u32::MAX),
        };
        
        let request = Arc::new(request);
        self.execute_with_retry(|connection| {
            let request = request.clone();
            async move {
                let conn = connection.lock().await;
                let response = conn.send_request((*request).clone()).await?;
            
                match response {
                Message::ReadFileResponse { 
                    success: true, 
                    data: Some(data), 
                    .. 
                } => {
                    // Update stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.bytes_read += data.len() as u64;
                    }
                    
                    Ok(Bytes::from(data))
                }
                Message::ReadFileResponse { 
                    success: false, 
                    error: Some(error), 
                    .. 
                } => {
                    Err(ClientError::RemoteFs(
                        remotefs_common::error::RemoteFsError::FileSystem(error)
                    ))
                }
                _ => Err(ClientError::InvalidResponse(
                    "Unexpected response for read file request".to_string()
                )),
                }
            }
        }).await
    }
    
    /// Write data to a file on the remote filesystem
    pub async fn write_file<P: AsRef<Path>>(
        &self,
        path: P,
        data: Bytes,
    ) -> ClientResult<()> {
        self.write_file_at(path, data, None, true).await
    }
    
    /// Write data to a file at a specific offset
    pub async fn write_file_at<P: AsRef<Path>>(
        &self,
        path: P,
        data: Bytes,
        offset: Option<u64>,
        sync: bool,
    ) -> ClientResult<()> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        let data_len = data.len();
        
        let request = Message::WriteFile {
            request_id: generate_request_id(),
            path: path_str.clone(),
            data: data.to_vec(),
            offset: offset.unwrap_or(0),
            sync,
        };
        
        let request = Arc::new(request);
        self.execute_with_retry(|connection| {
            let request = request.clone();
            async move {
                let conn = connection.lock().await;
                let response = conn.send_request((*request).clone()).await?;
            
            match response {
                Message::WriteFileResponse { 
                    success: true, 
                    .. 
                } => {
                    // Update stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.bytes_written += data_len as u64;
                    }
                    
                    Ok(())
                }
                Message::WriteFileResponse { 
                    success: false, 
                    error: Some(error), 
                    .. 
                } => {
                    Err(ClientError::RemoteFs(
                        remotefs_common::error::RemoteFsError::FileSystem(error)
                    ))
                }
                _ => Err(ClientError::InvalidResponse(
                    "Unexpected response for write file request".to_string()
                )),
            }
        }
        }).await
    }
    
    /// List directory contents
    pub async fn list_directory<P: AsRef<Path>>(&self, path: P) -> ClientResult<Vec<DirEntry>> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        
        let request = Message::ListDirectory {
            request_id: generate_request_id(),
            path: path_str.clone(),
        };
        
        let request = Arc::new(request);
        self.execute_with_retry(|connection| {
            let request = request.clone();
            async move {
                let conn = connection.lock().await;
                let response = conn.send_request((*request).clone()).await?;
            
            match response {
                Message::ListDirectoryResponse { 
                    success: true, 
                    entries: Some(entries), 
                    .. 
                } => Ok(entries),
                Message::ListDirectoryResponse { 
                    success: false, 
                    error: Some(error), 
                    .. 
                } => {
                    Err(ClientError::RemoteFs(
                        remotefs_common::error::RemoteFsError::FileSystem(error)
                    ))
                }
                _ => Err(ClientError::InvalidResponse(
                    "Unexpected response for list directory request".to_string()
                )),
            }
        }
        }).await
    }
    
    /// Get file or directory metadata
    pub async fn get_metadata<P: AsRef<Path>>(&self, path: P) -> ClientResult<FileMetadata> {
        self.get_metadata_with_options(path, true).await
    }
    
    /// Get file or directory metadata with options
    pub async fn get_metadata_with_options<P: AsRef<Path>>(
        &self,
        path: P,
        follow_symlinks: bool,
    ) -> ClientResult<FileMetadata> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        
        let request = Message::GetMetadata {
            request_id: generate_request_id(),
            path: path_str.clone(),
            follow_symlinks,
        };
        
        let request = Arc::new(request);
        self.execute_with_retry(|connection| {
            let request = request.clone();
            async move {
                let conn = connection.lock().await;
                let response = conn.send_request((*request).clone()).await?;
            
                match response {
                Message::GetMetadataResponse { 
                    success: true, 
                    metadata: Some(metadata), 
                    .. 
                } => Ok(metadata),
                Message::GetMetadataResponse { 
                    success: false, 
                    error: Some(error), 
                    .. 
                } => {
                    Err(ClientError::RemoteFs(
                        remotefs_common::error::RemoteFsError::FileSystem(error)
                    ))
                }
                _ => Err(ClientError::InvalidResponse(
                    "Unexpected response for get metadata request".to_string()
                )),
                }
            }
        }).await
    }
    
    /// Create a directory
    pub async fn create_directory<P: AsRef<Path>>(&self, path: P) -> ClientResult<()> {
        self.create_directory_with_mode(path, 0o755).await
    }
    
    /// Create a directory with specific permissions
    pub async fn create_directory_with_mode<P: AsRef<Path>>(
        &self,
        path: P,
        mode: u32,
    ) -> ClientResult<()> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        
        let request = Message::CreateDirectory {
            request_id: generate_request_id(),
            path: path_str.clone(),
            mode,
        };
        
        let request = Arc::new(request);
        self.execute_with_retry(|connection| {
            let request = request.clone();
            async move {
                let conn = connection.lock().await;
                let response = conn.send_request((*request).clone()).await?;
            
                match response {
                Message::CreateDirectoryResponse { 
                    success: true, 
                    .. 
                } => Ok(()),
                Message::CreateDirectoryResponse { 
                    success: false, 
                    error: Some(error), 
                    .. 
                } => {
                    Err(ClientError::RemoteFs(
                        remotefs_common::error::RemoteFsError::FileSystem(error)
                    ))
                }
                _ => Err(ClientError::InvalidResponse(
                    "Unexpected response for create directory request".to_string()
                )),
            }
        }
        }).await
    }
    
    /// Delete a file
    pub async fn delete_file<P: AsRef<Path>>(&self, path: P) -> ClientResult<()> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        
        let request = Message::DeleteFile {
            request_id: generate_request_id(),
            path: path_str.clone(),
        };
        
        let request = Arc::new(request);
        self.execute_with_retry(|connection| {
            let request = request.clone();
            async move {
                let conn = connection.lock().await;
                let response = conn.send_request((*request).clone()).await?;
            
                match response {
                Message::DeleteFileResponse { 
                    success: true, 
                    .. 
                } => Ok(()),
                Message::DeleteFileResponse { 
                    success: false, 
                    error: Some(error), 
                    .. 
                } => {
                    Err(ClientError::RemoteFs(
                        remotefs_common::error::RemoteFsError::FileSystem(error)
                    ))
                }
                _ => Err(ClientError::InvalidResponse(
                    "Unexpected response for delete file request".to_string()
                )),
            }
        }
        }).await
    }
    
    /// Delete a directory
    pub async fn delete_directory<P: AsRef<Path>>(&self, path: P) -> ClientResult<()> {
        let path_str = path.as_ref().to_string_lossy().to_string();
        
        let request = Message::RemoveDirectory {
            request_id: generate_request_id(),
            path: path_str.clone(),
            recursive: true,
        };
        
        let request = Arc::new(request);
        self.execute_with_retry(|connection| {
            let request = request.clone();
            async move {
                let conn = connection.lock().await;
                let response = conn.send_request((*request).clone()).await?;
            
                match response {
                Message::RemoveDirectoryResponse { 
                    success: true, 
                    .. 
                } => Ok(()),
                Message::RemoveDirectoryResponse { 
                    success: false, 
                    error: Some(error), 
                    .. 
                } => {
                    Err(ClientError::RemoteFs(
                        remotefs_common::error::RemoteFsError::FileSystem(error)
                    ))
                }
                _ => Err(ClientError::InvalidResponse(
                    "Unexpected response for delete directory request".to_string()
                )),
            }
        }
        }).await
    }
    
    /// Move/rename a file or directory
    pub async fn move_path<P: AsRef<Path>>(&self, source: P, destination: P) -> ClientResult<()> {
        let source_str = source.as_ref().to_string_lossy().to_string();
        let dest_str = destination.as_ref().to_string_lossy().to_string();
        
        let request = Message::Rename {
            request_id: generate_request_id(),
            from_path: source_str.clone(),
            to_path: dest_str.clone(),
        };
        
        let request = Arc::new(request);
        self.execute_with_retry(|connection| {
            let request = request.clone();
            async move {
                let conn = connection.lock().await;
                let response = conn.send_request((*request).clone()).await?;
            
                match response {
                Message::RenameResponse { 
                    success: true, 
                    .. 
                } => Ok(()),
                Message::RenameResponse { 
                    success: false, 
                    error: Some(error), 
                    .. 
                } => {
                    Err(ClientError::RemoteFs(
                        remotefs_common::error::RemoteFsError::FileSystem(error)
                    ))
                }
                _ => Err(ClientError::InvalidResponse(
                    "Unexpected response for rename request".to_string()
                )),
            }
        }
        }).await
    }
    
    /// Copy a file (implemented as read + write)
    pub async fn copy_file<P: AsRef<Path>>(&self, source: P, destination: P) -> ClientResult<()> {
        // Read the source file
        let data = self.read_file(source).await?;
        
        // Write to the destination file
        self.write_file(destination, data).await?;
        
        Ok(())
    }
    
    /// Get client statistics
    pub async fn get_stats(&self) -> ClientStats {
        self.stats.read().await.clone()
    }
    
    /// Get connection status for all agents
    pub async fn get_connection_status(&self) -> Vec<(String, ConnectionState)> {
        let connections = self.connection_pool.get_all_connections().await;
        let mut statuses = Vec::new();
        
        for connection in connections {
            let conn = connection.lock().await;
            let agent_id = conn.agent_config().id.clone();
            let state = conn.state().await;
            statuses.push((agent_id, state));
        }
        
        statuses
    }
    
    /// Execute an operation with retry logic and load balancing
    async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> ClientResult<T>
    where
        F: Fn(Arc<Mutex<AgentConnection>>) -> Fut,
        Fut: std::future::Future<Output = ClientResult<T>>,
    {
        let start_time = SystemTime::now();
        let mut last_error = None;
        
        for attempt in 0..=self.config.client.max_retries {
            // Get a connection from the pool
            match self.connection_pool.get_connection().await {
                Ok(connection) => {
                    match operation(connection).await {
                        Ok(result) => {
                            // Update success stats
                            {
                                let mut stats = self.stats.write().await;
                                stats.operations_successful += 1;
                                stats.operations_total += 1;
                                
                                if let Ok(elapsed) = start_time.elapsed() {
                                    let elapsed_ms = elapsed.as_millis() as f64;
                                    stats.avg_response_time_ms = 
                                        (stats.avg_response_time_ms * (stats.operations_successful - 1) as f64 + elapsed_ms) / 
                                        stats.operations_successful as f64;
                                }
                            }
                            
                            return Ok(result);
                        }
                        Err(e) if e.is_retryable() && attempt < self.config.client.max_retries => {
                            warn!("Retryable error on attempt {}: {}", attempt + 1, e);
                            last_error = Some(e);
                            
                            // Apply retry delay
                            if let Some(delay) = self.calculate_retry_delay(attempt) {
                                sleep(delay).await;
                            }
                            
                            continue;
                        }
                        Err(e) => {
                            last_error = Some(e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                    break;
                }
            }
        }
        
        // Update failure stats
        {
            let mut stats = self.stats.write().await;
            stats.operations_failed += 1;
            stats.operations_total += 1;
        }
        
        Err(last_error.unwrap_or_else(|| ClientError::Internal(
            "Operation failed without specific error".to_string()
        )))
    }
    
    /// Calculate retry delay based on strategy
    fn calculate_retry_delay(&self, attempt: u32) -> Option<Duration> {
        match &self.config.client.retry_strategy {
            RetryStrategy::None => None,
            RetryStrategy::Linear { delay_ms } => {
                Some(Duration::from_millis(*delay_ms))
            }
            RetryStrategy::Exponential { base_delay_ms, max_delay_ms } => {
                let delay = (*base_delay_ms as f64 * 2.0_f64.powi(attempt as i32)) as u64;
                let delay = delay.min(*max_delay_ms);
                Some(Duration::from_millis(delay))
            }
        }
    }
}

impl Drop for RemoteFsClient {
    fn drop(&mut self) {
        // Note: We can't call async methods in Drop, so we just clean up synchronously
        debug!("RemoteFsClient dropped");
    }
}
