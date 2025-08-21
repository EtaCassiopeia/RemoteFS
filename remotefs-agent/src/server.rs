use remotefs_common::{
    protocol::{Message, NodeType, generate_request_id},
    config::AgentConfig,
    error::{RemoteFsError, Result},
    crypto::{generate_keypair},
};
use crate::{
    connection::ConnectionManager,
    filesystem::FilesystemHandler,
    access::AccessControl,
};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Main agent server that connects to relay and handles filesystem operations
pub struct AgentServer {
    config: AgentConfig,
    connection_manager: Arc<ConnectionManager>,
    filesystem_handler: Arc<FilesystemHandler>,
    access_control: Arc<AccessControl>,
    shutdown_tx: broadcast::Sender<()>,
    shutdown_rx: broadcast::Receiver<()>,
    agent_id: String,
    public_key: Vec<u8>,
    private_key: Vec<u8>,
}

impl AgentServer {
    /// Create a new agent server with the given configuration
    pub fn new(config: AgentConfig) -> Result<Self> {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        
        // Generate agent keys for authentication
        let (private_key, public_key) = generate_keypair();
        
        // Create access control with the agent configuration
        let access_control = Arc::new(AccessControl::new(&config.access));
        
        // Create filesystem handler with access control
        let filesystem_handler = Arc::new(FilesystemHandler::new(
            Arc::clone(&access_control),
            &config.performance,
        ));
        
        // Create connection manager
        let connection_manager = Arc::new(ConnectionManager::new(
            &config,
            config.agent_id.clone(),
            public_key.clone(),
        )?);
        
        Ok(Self {
            agent_id: config.agent_id.clone(),
            config,
            connection_manager,
            filesystem_handler,
            access_control,
            shutdown_tx,
            shutdown_rx,
            public_key,
            private_key,
        })
    }
    
    /// Start the agent server
    pub async fn run(&self) -> Result<()> {
        info!("Starting RemoteFS Agent: {}", self.agent_id);
        info!("Connecting to relay: {}", self.config.relay_url);
        
        // Start connection to relay server
        let connection_handle = {
            let conn_mgr = Arc::clone(&self.connection_manager);
            let fs_handler = Arc::clone(&self.filesystem_handler);
            let mut shutdown_rx = self.shutdown_rx.resubscribe();
            
            tokio::spawn(async move {
                if let Err(e) = conn_mgr.connect_and_serve(fs_handler, shutdown_rx).await {
                    error!("Connection manager error: {}", e);
                }
            })
        };
        
        // Start health monitoring
        let health_handle = self.start_health_monitoring();
        
        // Start performance monitoring if enabled
        let perf_handle = self.start_performance_monitoring();
        
        // Start access log cleanup if enabled
        let cleanup_handle = self.start_cleanup_tasks();
        
        info!("RemoteFS Agent started and ready to serve filesystem operations");
        
        // Wait for shutdown signal
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal");
            }
            _ = connection_handle => {
                warn!("Connection manager ended unexpectedly");
            }
            _ = health_handle => {
                warn!("Health monitoring ended unexpectedly");
            }
            _ = perf_handle => {
                warn!("Performance monitoring ended unexpectedly");
            }
            _ = cleanup_handle => {
                warn!("Cleanup tasks ended unexpectedly");
            }
        }
        
        info!("Shutting down RemoteFS Agent");
        let _ = self.shutdown_tx.send(());
        
        // Give tasks a moment to shut down gracefully
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        
        Ok(())
    }
    
    /// Start health monitoring background task
    fn start_health_monitoring(&self) -> tokio::task::JoinHandle<()> {
        let connection_manager = Arc::clone(&self.connection_manager);
        let filesystem_handler = Arc::clone(&self.filesystem_handler);
        let mut shutdown_rx = self.shutdown_rx.resubscribe();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(30) // Health check every 30 seconds
            );
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Check connection health
                        if !connection_manager.is_connected().await {
                            warn!("Connection to relay is not healthy");
                        }
                        
                        // Check filesystem handler health
                        let fs_stats = filesystem_handler.get_statistics().await;
                        debug!(
                            "Filesystem stats - Active operations: {}, Total processed: {}, Errors: {}",
                            fs_stats.active_operations,
                            fs_stats.total_operations,
                            fs_stats.error_count
                        );
                        
                        // Log health status every 5 minutes
                        if fs_stats.total_operations > 0 && fs_stats.total_operations % 100 == 0 {
                            info!("Agent health - {} operations processed, {} errors", 
                                fs_stats.total_operations, fs_stats.error_count);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Health monitoring shutting down");
                        break;
                    }
                }
            }
        })
    }
    
    /// Start performance monitoring background task
    fn start_performance_monitoring(&self) -> tokio::task::JoinHandle<()> {
        let filesystem_handler = Arc::clone(&self.filesystem_handler);
        let mut shutdown_rx = self.shutdown_rx.resubscribe();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(300) // Report every 5 minutes
            );
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let stats = filesystem_handler.get_statistics().await;
                        let perf_stats = filesystem_handler.get_performance_stats().await;
                        
                        info!("Performance Report:");
                        info!("  Operations: {} total, {} active", stats.total_operations, stats.active_operations);
                        info!("  Errors: {} ({:.2}%)", stats.error_count, 
                            if stats.total_operations > 0 {
                                (stats.error_count as f64 / stats.total_operations as f64) * 100.0
                            } else {
                                0.0
                            });
                        info!("  Average response time: {:.2}ms", perf_stats.avg_response_time_ms);
                        info!("  Data transferred: {} bytes read, {} bytes written", 
                            perf_stats.bytes_read, perf_stats.bytes_written);
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Performance monitoring shutting down");
                        break;
                    }
                }
            }
        })
    }
    
    /// Start cleanup background tasks
    fn start_cleanup_tasks(&self) -> tokio::task::JoinHandle<()> {
        let filesystem_handler = Arc::clone(&self.filesystem_handler);
        let mut shutdown_rx = self.shutdown_rx.resubscribe();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(3600) // Cleanup every hour
            );
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // Cleanup temporary files and cached data
                        let cleaned = filesystem_handler.cleanup_temp_files().await;
                        if cleaned > 0 {
                            info!("Cleaned up {} temporary files", cleaned);
                        }
                        
                        // Cleanup old performance metrics
                        filesystem_handler.cleanup_old_metrics().await;
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Cleanup tasks shutting down");
                        break;
                    }
                }
            }
        })
    }
    
    /// Get agent status information
    pub async fn get_status(&self) -> AgentStatus {
        AgentStatus {
            agent_id: self.agent_id.clone(),
            connected: self.connection_manager.is_connected().await,
            uptime_seconds: self.connection_manager.get_uptime().await,
            filesystem_stats: self.filesystem_handler.get_statistics().await,
            connection_stats: self.connection_manager.get_statistics().await,
            access_control_stats: self.access_control.get_statistics().await,
        }
    }
}

/// Agent status information
#[derive(Debug, Clone)]
pub struct AgentStatus {
    pub agent_id: String,
    pub connected: bool,
    pub uptime_seconds: u64,
    pub filesystem_stats: FilesystemStatistics,
    pub connection_stats: ConnectionStatistics,
    pub access_control_stats: AccessControlStatistics,
}

/// Filesystem operation statistics
#[derive(Debug, Clone)]
pub struct FilesystemStatistics {
    pub active_operations: usize,
    pub total_operations: u64,
    pub error_count: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
}

/// Connection statistics
#[derive(Debug, Clone)]
pub struct ConnectionStatistics {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub reconnection_count: u32,
    pub last_heartbeat: Option<std::time::SystemTime>,
}

/// Access control statistics
#[derive(Debug, Clone)]
pub struct AccessControlStatistics {
    pub allowed_requests: u64,
    pub denied_requests: u64,
    pub path_violations: u64,
    pub size_violations: u64,
}

/// Performance statistics
#[derive(Debug, Clone)]
pub struct PerformanceStatistics {
    pub avg_response_time_ms: f64,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub operations_per_second: f64,
}
