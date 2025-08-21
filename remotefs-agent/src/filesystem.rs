use remotefs_common::{
    protocol::{Message, FileMetadata, DirEntry},
    error::RemoteFsError,
    config::{PerformanceConfig},
};
use crate::{
    access::AccessControl,
    server::{FilesystemStatistics, PerformanceStatistics},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH, Duration},
    io::{Read, Write, Seek, SeekFrom},
    fs::{self, File, OpenOptions},
    os::unix::fs::PermissionsExt,
};
use tokio::sync::RwLock;
use tracing::{debug};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Handles filesystem operations with access control and performance monitoring
pub struct FilesystemHandler {
    access_control: Arc<AccessControl>,
    stats: Arc<RwLock<FilesystemStatistics>>,
    performance_stats: Arc<RwLock<PerformanceStats>>,
    active_operations: Arc<RwLock<HashMap<Uuid, OperationInfo>>>,
    performance_config: PerformanceConfig,
}

/// Internal performance statistics tracking
#[derive(Debug, Clone)]
struct PerformanceStats {
    response_times: Vec<Duration>,
    bytes_read: u64,
    bytes_written: u64,
    operations_count: u64,
    last_cleanup: SystemTime,
}

/// Information about an active operation
#[derive(Debug, Clone)]
struct OperationInfo {
    operation_type: String,
    path: PathBuf,
    start_time: SystemTime,
}

impl FilesystemHandler {
    /// Create a new filesystem handler
    pub fn new(
        access_control: Arc<AccessControl>,
        performance_config: &PerformanceConfig,
    ) -> Self {
        let stats = Arc::new(RwLock::new(FilesystemStatistics {
            active_operations: 0,
            total_operations: 0,
            error_count: 0,
            bytes_read: 0,
            bytes_written: 0,
        }));
        
        let performance_stats = Arc::new(RwLock::new(PerformanceStats {
            response_times: Vec::new(),
            bytes_read: 0,
            bytes_written: 0,
            operations_count: 0,
            last_cleanup: SystemTime::now(),
        }));
        
        Self {
            access_control,
            stats,
            performance_stats,
            active_operations: Arc::new(RwLock::new(HashMap::new())),
            performance_config: performance_config.clone(),
        }
    }
    
    /// Handle read file operation
    pub async fn handle_read_file(
        &self,
        request_id: Uuid,
        path: String,
        offset: Option<u64>,
        length: Option<u64>,
    ) -> Option<Message> {
        let operation_id = Uuid::new_v4();
        let start_time = SystemTime::now();
        
        // Track operation
        self.start_operation(operation_id, "read_file", &path).await;
        
        let result = async {
            // Check access permissions
            self.access_control.check_read_access(&path).await?;
            
            let path_buf = PathBuf::from(&path);
            
            // Check if path exists and is a file
            if !path_buf.exists() {
                return Err(RemoteFsError::NotFound(format!("File not found: {}", path)));
            }
            
            if !path_buf.is_file() {
                return Err(RemoteFsError::InvalidPath(format!("Path is not a file: {}", path)));
            }
            
            // Open file for reading
            let mut file = File::open(&path_buf)
                .map_err(|e| RemoteFsError::FileSystem(format!("Failed to open file: {}", e)))?;
            
            // Seek to offset if specified
            if let Some(offset) = offset {
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| RemoteFsError::FileSystem(format!("Failed to seek: {}", e)))?;
            }
            
            // Read data
            let data = if let Some(length) = length {
                let mut buffer = vec![0u8; length as usize];
                let bytes_read = file.read(&mut buffer)
                    .map_err(|e| RemoteFsError::FileSystem(format!("Failed to read file: {}", e)))?;
                buffer.truncate(bytes_read);
                buffer
            } else {
                let mut buffer = Vec::new();
                file.read_to_end(&mut buffer)
                    .map_err(|e| RemoteFsError::FileSystem(format!("Failed to read file: {}", e)))?;
                buffer
            };
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.bytes_read += data.len() as u64;
                stats.total_operations += 1;
            }
            
            {
                let mut perf_stats = self.performance_stats.write().await;
                perf_stats.bytes_read += data.len() as u64;
            }
            
            Ok(Message::ReadFileResponse {
                request_id,
                success: true,
                data: Some(data.clone()),
                bytes_read: data.len() as u64,
                error: None,
            })
        }.await;
        
        // End operation tracking
        self.end_operation(operation_id, start_time).await;
        
        match result {
            Ok(response) => Some(response),
            Err(e) => {
                self.record_error().await;
                Some(Message::ReadFileResponse {
                    request_id,
                    success: false,
                    data: None,
                    bytes_read: 0,
                    error: Some(e.to_string()),
                })
            }
        }
    }
    
    /// Handle write file operation
    pub async fn handle_write_file(
        &self,
        request_id: Uuid,
        path: String,
        data: Vec<u8>,
        offset: Option<u64>,
        create: bool,
    ) -> Option<Message> {
        let operation_id = Uuid::new_v4();
        let start_time = SystemTime::now();
        
        // Track operation
        self.start_operation(operation_id, "write_file", &path).await;
        
        let result = async {
            // Check access permissions
            if create {
                self.access_control.check_create_access(&path).await?;
            } else {
                self.access_control.check_write_access(&path).await?;
            }
            
            let path_buf = PathBuf::from(&path);
            
            // Check file size limit
            let new_size = data.len() as u64 + offset.unwrap_or(0);
            self.access_control.check_file_size(new_size).await?;
            
            // Create parent directory if it doesn't exist and create is true
            if create {
                if let Some(parent) = path_buf.parent() {
                    if !parent.exists() {
                        fs::create_dir_all(parent)
                            .map_err(|e| RemoteFsError::FileSystem(format!("Failed to create parent directories: {}", e)))?;
                    }
                }
            }
            
            // Open file for writing
            let mut file = if create {
                OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(offset.is_none())
                    .open(&path_buf)
            } else {
                OpenOptions::new()
                    .write(true)
                    .open(&path_buf)
            }.map_err(|e| RemoteFsError::FileSystem(format!("Failed to open file for writing: {}", e)))?;
            
            // Seek to offset if specified
            if let Some(offset) = offset {
                file.seek(SeekFrom::Start(offset))
                    .map_err(|e| RemoteFsError::FileSystem(format!("Failed to seek: {}", e)))?;
            }
            
            // Write data
            file.write_all(&data)
                .map_err(|e| RemoteFsError::FileSystem(format!("Failed to write file: {}", e)))?;
            
            // Sync to disk if enabled
            if self.performance_config.async_io {
                file.sync_data()
                    .map_err(|e| RemoteFsError::FileSystem(format!("Failed to sync file: {}", e)))?;
            }
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.bytes_written += data.len() as u64;
                stats.total_operations += 1;
            }
            
            {
                let mut perf_stats = self.performance_stats.write().await;
                perf_stats.bytes_written += data.len() as u64;
            }
            
            Ok(Message::WriteFileResponse {
                request_id,
                success: true,
                bytes_written: data.len() as u64,
                error: None,
            })
        }.await;
        
        // End operation tracking
        self.end_operation(operation_id, start_time).await;
        
        match result {
            Ok(response) => Some(response),
            Err(e) => {
                self.record_error().await;
                Some(Message::WriteFileResponse {
                    request_id,
                    success: false,
                    bytes_written: 0,
                    error: Some(e.to_string()),
                })
            }
        }
    }
    
    /// Handle list directory operation
    pub async fn handle_list_directory(
        &self,
        request_id: Uuid,
        path: String,
    ) -> Option<Message> {
        let operation_id = Uuid::new_v4();
        let start_time = SystemTime::now();
        
        // Track operation
        self.start_operation(operation_id, "list_directory", &path).await;
        
        let result = async {
            // Check access permissions
            self.access_control.check_read_access(&path).await?;
            
            let path_buf = PathBuf::from(&path);
            
            // Check if path exists and is a directory
            if !path_buf.exists() {
                return Err(RemoteFsError::NotFound(format!("Directory not found: {}", path)));
            }
            
            if !path_buf.is_dir() {
                return Err(RemoteFsError::InvalidPath(format!("Path is not a directory: {}", path)));
            }
            
            // Read directory entries
            let entries = fs::read_dir(&path_buf)
                .map_err(|e| RemoteFsError::FileSystem(format!("Failed to read directory: {}", e)))?;
            
            let mut dir_entries = Vec::new();
            
            for entry in entries {
                let entry = entry
                    .map_err(|e| RemoteFsError::FileSystem(format!("Failed to read directory entry: {}", e)))?;
                
                let entry_path = entry.path();
                let metadata = entry.metadata()
                    .map_err(|e| RemoteFsError::FileSystem(format!("Failed to read metadata: {}", e)))?;
                
                let file_name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                
                // Create FileMetadata for this entry
                let file_metadata = FileMetadata {
                    size: metadata.len(),
                    modified: metadata.modified()
                        .map(|st| DateTime::from_timestamp(st.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64, 0).unwrap_or_default())
                        .unwrap_or_default(),
                    created: metadata.created()
                        .map(|st| DateTime::from_timestamp(st.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64, 0).unwrap_or_default())
                        .unwrap_or_default(),
                    accessed: metadata.accessed()
                        .map(|st| DateTime::from_timestamp(st.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64, 0).unwrap_or_default())
                        .unwrap_or_default(),
                    permissions: metadata.permissions().mode(),
                    uid: 0, // Default for cross-platform compatibility
                    gid: 0, // Default for cross-platform compatibility 
                    is_dir: metadata.is_dir(),
                    is_file: metadata.is_file(),
                    is_symlink: metadata.is_symlink(),
                    symlink_target: if metadata.is_symlink() {
                        entry_path.read_link().ok().and_then(|p| p.to_str().map(|s| s.to_string()))
                    } else {
                        None
                    },
                };
                
                let dir_entry = DirEntry {
                    name: file_name,
                    metadata: file_metadata,
                };
                
                dir_entries.push(dir_entry);
            }
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_operations += 1;
            }
            
            Ok(Message::DirectoryListing {
                request_id,
                entries: dir_entries,
                error: None,
            })
        }.await;
        
        // End operation tracking
        self.end_operation(operation_id, start_time).await;
        
        match result {
            Ok(response) => Some(response),
            Err(e) => {
                self.record_error().await;
                Some(Message::DirectoryListing {
                    request_id,
                    entries: vec![],
                    error: Some(e.to_string()),
                })
            }
        }
    }
    
    /// Handle get metadata operation
    pub async fn handle_get_metadata(
        &self,
        request_id: Uuid,
        path: String,
    ) -> Option<Message> {
        let operation_id = Uuid::new_v4();
        let start_time = SystemTime::now();
        
        // Track operation
        self.start_operation(operation_id, "get_metadata", &path).await;
        
        let result = async {
            // Check access permissions
            self.access_control.check_read_access(&path).await?;
            
            let path_buf = PathBuf::from(&path);
            
            // Check if path exists
            if !path_buf.exists() {
                return Err(RemoteFsError::NotFound(format!("Path not found: {}", path)));
            }
            
            // Get metadata
            let metadata = path_buf.metadata()
                .map_err(|e| RemoteFsError::FileSystem(format!("Failed to read metadata: {}", e)))?;
            
            let file_metadata = FileMetadata {
                size: metadata.len(),
                is_directory: metadata.is_dir(),
                is_readonly: metadata.permissions().readonly(),
                modified: metadata.modified()
                    .and_then(|t| t.duration_since(UNIX_EPOCH))
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                accessed: metadata.accessed()
                    .and_then(|t| t.duration_since(UNIX_EPOCH))
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                created: metadata.created()
                    .and_then(|t| t.duration_since(UNIX_EPOCH))
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            };
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_operations += 1;
            }
            
            Ok(Message::FileInfo {
                request_id,
                metadata: file_metadata,
                error: None,
            })
        }.await;
        
        // End operation tracking
        self.end_operation(operation_id, start_time).await;
        
        match result {
            Ok(response) => Some(response),
            Err(e) => {
                self.record_error().await;
                Some(Message::FileInfo {
                    request_id,
                    metadata: FileMetadata {
                        size: 0,
                        is_directory: false,
                        is_readonly: false,
                        modified: 0,
                        accessed: 0,
                        created: 0,
                    },
                    error: Some(e.to_string()),
                })
            }
        }
    }
    
    /// Handle create directory operation
    pub async fn handle_create_directory(
        &self,
        request_id: Uuid,
        path: String,
        recursive: bool,
    ) -> Option<Message> {
        let operation_id = Uuid::new_v4();
        let start_time = SystemTime::now();
        
        // Track operation
        self.start_operation(operation_id, "create_directory", &path).await;
        
        let result = async {
            // Check access permissions
            self.access_control.check_create_access(&path).await?;
            
            let path_buf = PathBuf::from(&path);
            
            // Create directory
            let result = if recursive {
                fs::create_dir_all(&path_buf)
            } else {
                fs::create_dir(&path_buf)
            };
            
            result.map_err(|e| RemoteFsError::FileSystem(format!("Failed to create directory: {}", e)))?;
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_operations += 1;
            }
            
            Ok(Message::OperationResult {
                request_id,
                success: true,
                error: None,
            })
        }.await;
        
        // End operation tracking
        self.end_operation(operation_id, start_time).await;
        
        match result {
            Ok(response) => Some(response),
            Err(e) => {
                self.record_error().await;
                Some(Message::OperationResult {
                    request_id,
                    success: false,
                    error: Some(e.to_string()),
                })
            }
        }
    }
    
    /// Handle delete file operation
    pub async fn handle_delete_file(
        &self,
        request_id: Uuid,
        path: String,
    ) -> Option<Message> {
        let operation_id = Uuid::new_v4();
        let start_time = SystemTime::now();
        
        // Track operation
        self.start_operation(operation_id, "delete_file", &path).await;
        
        let result = async {
            // Check access permissions
            self.access_control.check_delete_access(&path).await?;
            
            let path_buf = PathBuf::from(&path);
            
            // Check if path exists and is a file
            if !path_buf.exists() {
                return Err(RemoteFsError::NotFound(format!("File not found: {}", path)));
            }
            
            if !path_buf.is_file() {
                return Err(RemoteFsError::InvalidPath(format!("Path is not a file: {}", path)));
            }
            
            // Delete file
            fs::remove_file(&path_buf)
                .map_err(|e| RemoteFsError::FileSystem(format!("Failed to delete file: {}", e)))?;
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_operations += 1;
            }
            
            Ok(Message::OperationResult {
                request_id,
                success: true,
                error: None,
            })
        }.await;
        
        // End operation tracking
        self.end_operation(operation_id, start_time).await;
        
        match result {
            Ok(response) => Some(response),
            Err(e) => {
                self.record_error().await;
                Some(Message::OperationResult {
                    request_id,
                    success: false,
                    error: Some(e.to_string()),
                })
            }
        }
    }
    
    /// Handle delete directory operation
    pub async fn handle_delete_directory(
        &self,
        request_id: Uuid,
        path: String,
        recursive: bool,
    ) -> Option<Message> {
        let operation_id = Uuid::new_v4();
        let start_time = SystemTime::now();
        
        // Track operation
        self.start_operation(operation_id, "delete_directory", &path).await;
        
        let result = async {
            // Check access permissions
            self.access_control.check_delete_access(&path).await?;
            
            let path_buf = PathBuf::from(&path);
            
            // Check if path exists and is a directory
            if !path_buf.exists() {
                return Err(RemoteFsError::NotFound(format!("Directory not found: {}", path)));
            }
            
            if !path_buf.is_dir() {
                return Err(RemoteFsError::InvalidPath(format!("Path is not a directory: {}", path)));
            }
            
            // Delete directory
            let result = if recursive {
                fs::remove_dir_all(&path_buf)
            } else {
                fs::remove_dir(&path_buf)
            };
            
            result.map_err(|e| RemoteFsError::FileSystem(format!("Failed to delete directory: {}", e)))?;
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_operations += 1;
            }
            
            Ok(Message::OperationResult {
                request_id,
                success: true,
                error: None,
            })
        }.await;
        
        // End operation tracking
        self.end_operation(operation_id, start_time).await;
        
        match result {
            Ok(response) => Some(response),
            Err(e) => {
                self.record_error().await;
                Some(Message::OperationResult {
                    request_id,
                    success: false,
                    error: Some(e.to_string()),
                })
            }
        }
    }
    
    /// Handle move file operation
    pub async fn handle_move_file(
        &self,
        request_id: Uuid,
        source_path: String,
        dest_path: String,
    ) -> Option<Message> {
        let operation_id = Uuid::new_v4();
        let start_time = SystemTime::now();
        
        // Track operation
        self.start_operation(operation_id, "move_file", &source_path).await;
        
        let result = async {
            // Check access permissions
            self.access_control.check_read_access(&source_path).await?;
            self.access_control.check_create_access(&dest_path).await?;
            self.access_control.check_delete_access(&source_path).await?;
            
            let source_buf = PathBuf::from(&source_path);
            let dest_buf = PathBuf::from(&dest_path);
            
            // Check if source exists
            if !source_buf.exists() {
                return Err(RemoteFsError::NotFound(format!("Source not found: {}", source_path)));
            }
            
            // Create destination directory if needed
            if let Some(parent) = dest_buf.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|e| RemoteFsError::FileSystem(format!("Failed to create destination directories: {}", e)))?;
                }
            }
            
            // Move file/directory
            fs::rename(&source_buf, &dest_buf)
                .map_err(|e| RemoteFsError::FileSystem(format!("Failed to move: {}", e)))?;
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.total_operations += 1;
            }
            
            Ok(Message::OperationResult {
                request_id,
                success: true,
                error: None,
            })
        }.await;
        
        // End operation tracking
        self.end_operation(operation_id, start_time).await;
        
        match result {
            Ok(response) => Some(response),
            Err(e) => {
                self.record_error().await;
                Some(Message::OperationResult {
                    request_id,
                    success: false,
                    error: Some(e.to_string()),
                })
            }
        }
    }
    
    /// Handle copy file operation
    pub async fn handle_copy_file(
        &self,
        request_id: Uuid,
        source_path: String,
        dest_path: String,
    ) -> Option<Message> {
        let operation_id = Uuid::new_v4();
        let start_time = SystemTime::now();
        
        // Track operation
        self.start_operation(operation_id, "copy_file", &source_path).await;
        
        let result = async {
            // Check access permissions
            self.access_control.check_read_access(&source_path).await?;
            self.access_control.check_create_access(&dest_path).await?;
            
            let source_buf = PathBuf::from(&source_path);
            let dest_buf = PathBuf::from(&dest_path);
            
            // Check if source exists and is a file
            if !source_buf.exists() {
                return Err(RemoteFsError::NotFound(format!("Source not found: {}", source_path)));
            }
            
            if !source_buf.is_file() {
                return Err(RemoteFsError::InvalidPath(format!("Source is not a file: {}", source_path)));
            }
            
            // Check file size limit
            let file_size = source_buf.metadata()
                .map_err(|e| RemoteFsError::FileSystem(format!("Failed to get source metadata: {}", e)))?
                .len();
            
            self.access_control.check_file_size(file_size).await?;
            
            // Create destination directory if needed
            if let Some(parent) = dest_buf.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|e| RemoteFsError::FileSystem(format!("Failed to create destination directories: {}", e)))?;
                }
            }
            
            // Copy file
            fs::copy(&source_buf, &dest_buf)
                .map_err(|e| RemoteFsError::FileSystem(format!("Failed to copy file: {}", e)))?;
            
            // Update statistics
            {
                let mut stats = self.stats.write().await;
                stats.bytes_read += file_size;
                stats.bytes_written += file_size;
                stats.total_operations += 1;
            }
            
            {
                let mut perf_stats = self.performance_stats.write().await;
                perf_stats.bytes_read += file_size;
                perf_stats.bytes_written += file_size;
            }
            
            Ok(Message::OperationResult {
                request_id,
                success: true,
                error: None,
            })
        }.await;
        
        // End operation tracking
        self.end_operation(operation_id, start_time).await;
        
        match result {
            Ok(response) => Some(response),
            Err(e) => {
                self.record_error().await;
                Some(Message::OperationResult {
                    request_id,
                    success: false,
                    error: Some(e.to_string()),
                })
            }
        }
    }
    
    /// Start tracking an operation
    async fn start_operation(&self, operation_id: Uuid, operation_type: &str, path: &str) {
        let operation_info = OperationInfo {
            operation_type: operation_type.to_string(),
            path: PathBuf::from(path),
            start_time: SystemTime::now(),
        };
        
        {
            let mut active = self.active_operations.write().await;
            active.insert(operation_id, operation_info);
        }
        
        {
            let mut stats = self.stats.write().await;
            stats.active_operations += 1;
        }
        
        debug!("Started {} operation on {}", operation_type, path);
    }
    
    /// End tracking an operation
    async fn end_operation(&self, operation_id: Uuid, start_time: SystemTime) {
        {
            let mut active = self.active_operations.write().await;
            active.remove(&operation_id);
        }
        
        {
            let mut stats = self.stats.write().await;
            stats.active_operations = stats.active_operations.saturating_sub(1);
        }
        
        // Record performance metrics
        if let Ok(duration) = start_time.elapsed() {
            let mut perf_stats = self.performance_stats.write().await;
            perf_stats.response_times.push(duration);
            perf_stats.operations_count += 1;
        }
    }
    
    /// Record an error
    async fn record_error(&self) {
        let mut stats = self.stats.write().await;
        stats.error_count += 1;
    }
    
    /// Get filesystem statistics
    pub async fn get_statistics(&self) -> FilesystemStatistics {
        self.stats.read().await.clone()
    }
    
    /// Get performance statistics
    pub async fn get_performance_stats(&self) -> PerformanceStatistics {
        let perf_stats = self.performance_stats.read().await;
        let stats = self.stats.read().await;
        
        let avg_response_time = if !perf_stats.response_times.is_empty() {
            let total_ms: u128 = perf_stats.response_times.iter()
                .map(|d| d.as_millis())
                .sum();
            total_ms as f64 / perf_stats.response_times.len() as f64
        } else {
            0.0
        };
        
        let ops_per_second = if perf_stats.operations_count > 0 {
            let uptime_seconds = perf_stats.last_cleanup.elapsed()
                .unwrap_or_else(|_| Duration::from_secs(1))
                .as_secs_f64();
            perf_stats.operations_count as f64 / uptime_seconds.max(1.0)
        } else {
            0.0
        };
        
        PerformanceStatistics {
            avg_response_time_ms: avg_response_time,
            bytes_read: stats.bytes_read,
            bytes_written: stats.bytes_written,
            operations_per_second: ops_per_second,
        }
    }
    
    /// Clean up temporary files (placeholder)
    pub async fn cleanup_temp_files(&self) -> usize {
        // TODO: Implement temporary file cleanup logic
        0
    }
    
    /// Clean up old performance metrics
    pub async fn cleanup_old_metrics(&self) {
        let mut perf_stats = self.performance_stats.write().await;
        
        // Keep only recent response times (last 1000 operations)
        if perf_stats.response_times.len() > 1000 {
            let start_index = perf_stats.response_times.len() - 1000;
            perf_stats.response_times.drain(..start_index);
        }
        
        perf_stats.last_cleanup = SystemTime::now();
    }
}
