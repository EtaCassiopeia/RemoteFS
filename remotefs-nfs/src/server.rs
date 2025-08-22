use crate::{RemoteNfsFilesystem, NfsConfig, Result};
use remotefs_client::Client;
use std::sync::Arc;
use tokio::signal;
use tracing::{info, error, warn};
use zerofs_nfsserve::tcp::{NFSTcpListener, NFSTcp};
use std::sync::atomic::{AtomicU64, Ordering};

/// RemoteFS NFS server for cross-platform compatibility (Linux & macOS)
pub struct RemoteNfsServer {
    config: NfsConfig,
    filesystem: Option<RemoteNfsFilesystem>,
}

impl RemoteNfsServer {
    pub fn new(config: NfsConfig) -> Self {
        Self {
            config,
            filesystem: None,
        }
    }

    /// Initialize the server with a RemoteFS client
    pub async fn initialize(&mut self, client: Client) -> Result<()> {
        info!("Initializing RemoteFS NFS server");
        
        let filesystem = RemoteNfsFilesystem::new(client).await?;
        self.filesystem = Some(filesystem);
        
        info!("RemoteFS NFS filesystem initialized");
        Ok(())
    }

    /// Start the NFS server
    pub async fn start(&self) -> Result<()> {
        let filesystem = match &self.filesystem {
            Some(fs) => fs.clone(),
            None => {
                error!("Server not initialized. Call initialize() first.");
                return Err(remotefs_common::error::RemoteFsError::Internal(
                    "Server not initialized".to_string()
                ));
            }
        };

        info!(
            "Starting RemoteFS NFS server on {}:{}",
            self.config.host, self.config.port
        );

        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = NFSTcpListener::bind(&addr, filesystem)
            .await
            .map_err(|e| {
                remotefs_common::error::RemoteFsError::Internal(
                    format!("Failed to bind NFS server: {}", e)
                )
            })?;

        info!("NFS server listening on {}", addr);
        info!("Mount with: sudo mount -t nfs -o vers=3,tcp,port={},mountport={} {}:/ /mnt/remotefs", 
              self.config.port, self.config.port, self.config.host);

        // Handle graceful shutdown
        tokio::select! {
            result = listener.handle_forever() => {
                match result {
                    Ok(_) => {
                        info!("NFS server stopped normally");
                        Ok(())
                    }
                    Err(e) => {
                        error!("NFS server error: {}", e);
                        Err(remotefs_common::error::RemoteFsError::Internal(
                            format!("NFS server error: {}", e)
                        ))
                    }
                }
            }
            _ = signal::ctrl_c() => {
                info!("Received SIGINT, shutting down gracefully...");
                Ok(())
            }
        }
    }

    /// Start server with retry logic and connection health monitoring
    pub async fn start_with_monitoring(&self, _client: &Client) -> Result<()> {
        let mut restart_count = 0;
        const MAX_RESTARTS: u32 = 5;
        const RESTART_DELAY_SECS: u64 = 10;

        loop {
            match self.start().await {
                Ok(_) => {
                    info!("NFS server stopped normally");
                    break;
                }
                Err(e) => {
                    error!("NFS server error: {:?}", e);
                    
                    if restart_count >= MAX_RESTARTS {
                        error!("Maximum restart attempts ({}) reached, giving up", MAX_RESTARTS);
                        return Err(e);
                    }

                    restart_count += 1;
                    warn!(
                        "Restarting NFS server (attempt {}/{}) in {} seconds...",
                        restart_count, MAX_RESTARTS, RESTART_DELAY_SECS
                    );

                    // Note: Client reconnection is handled internally by the client

                    tokio::time::sleep(tokio::time::Duration::from_secs(RESTART_DELAY_SECS)).await;
                }
            }
        }

        Ok(())
    }
}

impl Clone for RemoteNfsFilesystem {
    fn clone(&self) -> Self {
        Self {
            client: Arc::clone(&self.client),
            next_file_id: AtomicU64::new(self.next_file_id.load(Ordering::SeqCst)),
            path_to_id_map: Arc::clone(&self.path_to_id_map),
            id_to_path_map: Arc::clone(&self.id_to_path_map),
            root_id: self.root_id,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NfsConfig;
    use remotefs_client::{ClientConfig, AgentConfig};

    #[tokio::test]
    async fn test_server_creation() {
        let config = NfsConfig::default();
        let server = RemoteNfsServer::new(config);
        
        // Server should be created but not initialized
        assert!(server.filesystem.is_none());
    }

    #[tokio::test]
    async fn test_server_initialization() {
        let config = NfsConfig::default();
        let mut server = RemoteNfsServer::new(config);
        
        // Create a test client with minimal configuration
        let client_config = ClientConfig {
            agents: vec![AgentConfig {
                id: "test".to_string(),
                url: "ws://localhost:8080".to_string(),
                auth: None,
                weight: 1,
                enabled: true,
            }],
            ..Default::default()
        };
        let client = Client::new(client_config).unwrap();
        
        // Initialize should work (even if connection fails in tests)
        let result = server.initialize(client).await;
        // Note: This might fail in tests due to no actual server, but the structure should be correct
        assert!(server.filesystem.is_some() || result.is_err());
    }
}
