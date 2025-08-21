use remotefs_common::{
    protocol::{Message, NodeType, generate_request_id},
    config::AgentConfig,
    error::{RemoteFsError, Result},
};
use crate::{
    filesystem::FilesystemHandler,
    server::{ConnectionStatistics, PerformanceStatistics},
};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, mpsc};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};
use futures::{SinkExt, StreamExt};
use tracing::{info, warn, error, debug};
use url::Url;

/// Manages the WebSocket connection to the relay server
pub struct ConnectionManager {
    config: AgentConfig,
    agent_id: String,
    public_key: Vec<u8>,
    relay_url: Url,
    stats: Arc<RwLock<ConnectionStatistics>>,
    start_time: std::time::SystemTime,
}

impl ConnectionManager {
    /// Create a new connection manager
    pub fn new(
        config: &AgentConfig,
        agent_id: String,
        public_key: Vec<u8>,
    ) -> Result<Self> {
        let relay_url = Url::parse(&config.relay_url)
            .map_err(|e| RemoteFsError::Configuration(format!("Invalid relay URL: {}", e)))?;
        
        let stats = Arc::new(RwLock::new(ConnectionStatistics {
            messages_sent: 0,
            messages_received: 0,
            reconnection_count: 0,
            last_heartbeat: None,
        }));
        
        Ok(Self {
            config: config.clone(),
            agent_id,
            public_key,
            relay_url,
            stats,
            start_time: std::time::SystemTime::now(),
        })
    }
    
    /// Connect to relay and serve filesystem operations
    pub async fn connect_and_serve(
        &self,
        filesystem_handler: Arc<FilesystemHandler>,
        mut shutdown_rx: broadcast::Receiver<()>,
    ) -> Result<()> {
        let mut reconnect_attempts = 0;
        let max_attempts = self.config.network.max_reconnect_attempts;
        let base_delay = self.config.network.reconnect_backoff_base;
        
        loop {
            match self.try_connect_and_serve(Arc::clone(&filesystem_handler), &mut shutdown_rx).await {
                Ok(_) => {
                    info!("Connection closed normally");
                    break;
                }
                Err(e) => {
                    error!("Connection error: {}", e);
                    
                    reconnect_attempts += 1;
                    if reconnect_attempts >= max_attempts {
                        error!("Max reconnection attempts ({}) reached, giving up", max_attempts);
                        return Err(e);
                    }
                    
                    // Update reconnection stats
                    {
                        let mut stats = self.stats.write().await;
                        stats.reconnection_count += 1;
                    }
                    
                    // Exponential backoff
                    let delay = std::cmp::min(base_delay * (2_u64.pow(reconnect_attempts - 1)), 300);
                    warn!("Reconnecting in {} seconds (attempt {}/{})", delay, reconnect_attempts, max_attempts);
                    
                    // Check for shutdown during delay
                    tokio::select! {
                        _ = tokio::time::sleep(tokio::time::Duration::from_secs(delay)) => {}
                        _ = shutdown_rx.recv() => {
                            info!("Shutdown requested during reconnection delay");
                            return Ok(());
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Attempt a single connection and serve until disconnected
    async fn try_connect_and_serve(
        &self,
        filesystem_handler: Arc<FilesystemHandler>,
        shutdown_rx: &mut broadcast::Receiver<()>,
    ) -> Result<()> {
        info!("Connecting to relay server: {}", self.relay_url);
        
        // Connect to WebSocket
        let (ws_stream, _) = connect_async(&self.relay_url).await
            .map_err(|e| RemoteFsError::Connection(format!("Failed to connect to relay: {}", e)))?;
        
        info!("Connected to relay server");
        
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        
        // Create channels for internal communication
        let (message_tx, mut message_rx) = mpsc::unbounded_channel::<Message>();
        
        // Send authentication message
        let auth_message = Message::AuthRequest {
            node_id: self.agent_id.clone(),
            node_type: NodeType::Agent,
            public_key: self.public_key.clone(),
            capabilities: vec!["filesystem".to_string(), "read".to_string(), "write".to_string()],
        };
        
        let auth_json = serde_json::to_string(&auth_message)
            .map_err(|e| RemoteFsError::Protocol(format!("Failed to serialize auth message: {}", e)))?;
        
        ws_sender.send(WsMessage::Text(auth_json)).await
            .map_err(|e| RemoteFsError::Network(format!("Failed to send auth message: {}", e)))?;
        
        // Update stats
        {
            let mut stats = self.stats.write().await;
            stats.messages_sent += 1;
        }
        
        // Wait for authentication response
        if let Some(msg) = ws_receiver.next().await {
            let msg = msg.map_err(|e| RemoteFsError::Network(format!("WebSocket error: {}", e)))?;
            
            match msg {
                WsMessage::Text(text) => {
                    let response: Message = serde_json::from_str(&text)
                        .map_err(|e| RemoteFsError::Protocol(format!("Invalid auth response: {}", e)))?;
                    
                    if let Message::AuthResponse { success, error, .. } = response {
                        if success {
                            info!("Authentication successful");
                        } else {
                            let error_msg = error.unwrap_or_else(|| "Unknown auth error".to_string());
                            return Err(RemoteFsError::Authentication(format!("Auth failed: {}", error_msg)));
                        }
                    } else {
                        return Err(RemoteFsError::Protocol("Expected auth response".to_string()));
                    }
                }
                _ => return Err(RemoteFsError::Protocol("Expected text auth response".to_string())),
            }
            
            // Update stats
            {
                let mut stats = self.stats.write().await;
                stats.messages_received += 1;
            }
        } else {
            return Err(RemoteFsError::Connection("Connection closed during auth".to_string()));
        }
        
        // Start message sender task
        let sender_handle = {
            let mut ws_sender = ws_sender;
            let stats = Arc::clone(&self.stats);
            tokio::spawn(async move {
                while let Some(message) = message_rx.recv().await {
                    let result = match serde_json::to_string(&message) {
                        Ok(json) => ws_sender.send(WsMessage::Text(json)).await,
                        Err(e) => {
                            error!("Failed to serialize message: {}", e);
                            continue;
                        }
                    };
                    
                    if let Err(e) = result {
                        error!("Failed to send message: {}", e);
                        break;
                    }
                    
                    // Update stats
                    let mut stats = stats.write().await;
                    stats.messages_sent += 1;
                }
                debug!("Message sender task ended");
            })
        };
        
        // Start heartbeat task
        let heartbeat_handle = {
            let message_tx = message_tx.clone();
            let stats = Arc::clone(&self.stats);
            let heartbeat_interval = self.config.network.heartbeat_interval;
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(
                    tokio::time::Duration::from_secs(heartbeat_interval)
                );
                
                loop {
                    interval.tick().await;
                    
                    let ping_message = Message::Ping {
                        timestamp: chrono::Utc::now(),
                    };
                    
                    if message_tx.send(ping_message).is_err() {
                        debug!("Failed to send heartbeat - channel closed");
                        break;
                    }
                    
                    // Update heartbeat timestamp
                    let mut stats = stats.write().await;
                    stats.last_heartbeat = Some(std::time::SystemTime::now());
                }
                debug!("Heartbeat task ended");
            })
        };
        
        // Message handling loop
        loop {
            tokio::select! {
                // Handle incoming messages
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(WsMessage::Text(text))) => {
                            // Update stats
                            {
                                let mut stats = self.stats.write().await;
                                stats.messages_received += 1;
                            }
                            
                            match serde_json::from_str::<Message>(&text) {
                                Ok(message) => {
                                    if let Err(e) = self.handle_message(
                                        message,
                                        Arc::clone(&filesystem_handler),
                                        &message_tx
                                    ).await {
                                        error!("Error handling message: {}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to parse message: {}", e);
                                }
                            }
                        }
                        Some(Ok(WsMessage::Binary(data))) => {
                            // Update stats
                            {
                                let mut stats = self.stats.write().await;
                                stats.messages_received += 1;
                            }
                            
                            match bincode::deserialize::<Message>(&data) {
                                Ok(message) => {
                                    if let Err(e) = self.handle_message(
                                        message,
                                        Arc::clone(&filesystem_handler),
                                        &message_tx
                                    ).await {
                                        error!("Error handling binary message: {}", e);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to parse binary message: {}", e);
                                }
                            }
                        }
                        Some(Ok(WsMessage::Pong(_))) => {
                            debug!("Received pong from relay");
                        }
                        Some(Ok(WsMessage::Close(_))) => {
                            info!("Relay closed connection");
                            break;
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            warn!("WebSocket stream ended");
                            break;
                        }
                        _ => {} // Ignore other message types
                    }
                }
                
                // Handle shutdown signal
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    break;
                }
            }
        }
        
        // Clean up tasks
        sender_handle.abort();
        heartbeat_handle.abort();
        
        Ok(())
    }
    
    /// Handle an incoming message from the relay
    async fn handle_message(
        &self,
        message: Message,
        filesystem_handler: Arc<FilesystemHandler>,
        response_tx: &mpsc::UnboundedSender<Message>,
    ) -> Result<()> {
        debug!("Handling message: {:?}", message.message_type());
        
        let response = match message {
            Message::Pong { .. } => {
                debug!("Received pong from relay");
                return Ok(());
            }
            
            // Filesystem operations
            Message::ReadFile { request_id, path, offset, length } => {
                filesystem_handler.handle_read_file(request_id, path, Some(offset), Some(length as u64)).await
            }
            
            Message::WriteFile { request_id, path, data, offset, sync } => {
                filesystem_handler.handle_write_file(request_id, path, data, Some(offset), sync).await
            }
            
            Message::ListDirectory { request_id, path } => {
                filesystem_handler.handle_list_directory(request_id, path).await
            }
            
            Message::GetMetadata { request_id, path, follow_symlinks } => {
                filesystem_handler.handle_get_metadata(request_id, path, follow_symlinks).await
            }
            
            Message::CreateDirectory { request_id, path, mode } => {
                filesystem_handler.handle_create_directory(request_id, path, mode).await
            }
            
            Message::DeleteFile { request_id, path } => {
                filesystem_handler.handle_delete_file(request_id, path).await
            }
            
            Message::RemoveDirectory { request_id, path, recursive } => {
                filesystem_handler.handle_delete_directory(request_id, path, recursive).await
            }
            
            Message::Rename { request_id, from_path, to_path } => {
                filesystem_handler.handle_move_file(request_id, from_path, to_path).await
            }
            
            // Other messages that don't require responses
            _ => {
                debug!("Ignoring message type: {:?}", message.message_type());
                return Ok(());
            }
        };
        
        // Send response if we have one
        if let Some(response) = response {
            response_tx.send(response)
                .map_err(|_| RemoteFsError::Internal("Failed to send response".to_string()))?;
        }
        
        Ok(())
    }
    
    /// Check if connected to relay
    pub async fn is_connected(&self) -> bool {
        // Simple check based on recent heartbeat
        if let Some(last_heartbeat) = self.stats.read().await.last_heartbeat {
            let elapsed = last_heartbeat.elapsed().unwrap_or_else(|_| std::time::Duration::from_secs(u64::MAX));
            elapsed < std::time::Duration::from_secs(self.config.network.heartbeat_interval * 3)
        } else {
            false
        }
    }
    
    /// Get connection uptime in seconds
    pub async fn get_uptime(&self) -> u64 {
        self.start_time.elapsed()
            .unwrap_or_else(|_| std::time::Duration::from_secs(0))
            .as_secs()
    }
    
    /// Get connection statistics
    pub async fn get_statistics(&self) -> ConnectionStatistics {
        self.stats.read().await.clone()
    }
}
