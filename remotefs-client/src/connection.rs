use crate::config::{AgentConfig, ConnectionConfig};
use crate::error::{ClientError, ClientResult};
use remotefs_common::protocol::Message;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot, RwLock, Mutex};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage, WebSocketStream};
use futures::{SinkExt, StreamExt};
use tracing::{debug, error, info, warn};
use dashmap::DashMap;
use uuid::Uuid;

/// Connection state
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

/// Connection statistics
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub connection_attempts: u64,
    pub successful_connections: u64,
    pub failed_connections: u64,
    pub last_connected: Option<Instant>,
    pub last_disconnected: Option<Instant>,
    pub total_uptime: Duration,
}

/// Response waiter for request-response pattern
type ResponseWaiter = oneshot::Sender<ClientResult<Message>>;

/// WebSocket connection to a RemoteFS agent
pub struct AgentConnection {
    /// Agent configuration
    config: AgentConfig,
    
    /// Connection configuration
    connection_config: ConnectionConfig,
    
    /// Current connection state
    state: Arc<RwLock<ConnectionState>>,
    
    /// Connection statistics
    stats: Arc<RwLock<ConnectionStats>>,
    
    /// Pending requests waiting for responses
    pending_requests: Arc<DashMap<Uuid, ResponseWaiter>>,
    
    /// Channel for sending messages to the connection task
    message_sender: Option<mpsc::UnboundedSender<Message>>,
    
    /// Shutdown signal
    shutdown_tx: Option<oneshot::Sender<()>>,
    
    /// Task handles
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl AgentConnection {
    /// Create a new agent connection
    pub fn new(agent_config: AgentConfig, connection_config: ConnectionConfig) -> Self {
        Self {
            config: agent_config,
            connection_config,
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            stats: Arc::new(RwLock::new(ConnectionStats::default())),
            pending_requests: Arc::new(DashMap::new()),
            message_sender: None,
            shutdown_tx: None,
            tasks: Vec::new(),
        }
    }
    
    /// Connect to the agent
    pub async fn connect(&mut self) -> ClientResult<()> {
        if self.is_connected().await {
            return Ok(());
        }
        
        self.set_state(ConnectionState::Connecting).await;
        
        info!("Connecting to agent {} at {}", self.config.id, self.config.url);
        
        // Update connection attempt stats
        {
            let mut stats = self.stats.write().await;
            stats.connection_attempts += 1;
        }
        
        match self.establish_connection().await {
            Ok((message_sender, shutdown_tx, tasks)) => {
                self.message_sender = Some(message_sender);
                self.shutdown_tx = Some(shutdown_tx);
                self.tasks = tasks;
                
                self.set_state(ConnectionState::Connected).await;
                
                // Update success stats
                {
                    let mut stats = self.stats.write().await;
                    stats.successful_connections += 1;
                    stats.last_connected = Some(Instant::now());
                }
                
                info!("Successfully connected to agent {}", self.config.id);
                Ok(())
            }
            Err(e) => {
                self.set_state(ConnectionState::Failed).await;
                
                // Update failure stats
                {
                    let mut stats = self.stats.write().await;
                    stats.failed_connections += 1;
                }
                
                error!("Failed to connect to agent {}: {}", self.config.id, e);
                Err(e)
            }
        }
    }
    
    /// Disconnect from the agent
    pub async fn disconnect(&mut self) -> ClientResult<()> {
        if !self.is_connected().await {
            return Ok(());
        }
        
        info!("Disconnecting from agent {}", self.config.id);
        
        // Send shutdown signal
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        
        // Wait for tasks to complete
        for task in self.tasks.drain(..) {
            let _ = task.await;
        }
        
        self.message_sender = None;
        self.set_state(ConnectionState::Disconnected).await;
        
        // Update disconnect stats
        {
            let mut stats = self.stats.write().await;
            stats.last_disconnected = Some(Instant::now());
            if let Some(connected_at) = stats.last_connected {
                stats.total_uptime += Instant::now().duration_since(connected_at);
            }
        }
        
        info!("Disconnected from agent {}", self.config.id);
        Ok(())
    }
    
    /// Send a message and wait for response
    pub async fn send_request(&self, message: Message) -> ClientResult<Message> {
        let request_id = message.request_id();
        
        // Set up response waiter
        let (response_tx, response_rx) = oneshot::channel();
        if let Some(id) = request_id {
            self.pending_requests.insert(id, response_tx);
        }
        
        // Send the message
        self.send_message(message).await?;
        
        // Wait for response with timeout
        let response = timeout(
            self.connection_config.operation_timeout(),
            response_rx
        ).await;
        
        // Clean up pending request
        if let Some(id) = request_id {
            self.pending_requests.remove(&id);
        }
        
        match response {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => Err(ClientError::Internal(format!("Response channel error: {}", e))),
            Err(_) => Err(ClientError::Timeout { 
                seconds: self.connection_config.connect_timeout_ms / 1000 
            }),
        }
    }
    
    /// Send a message without waiting for response
    pub async fn send_message(&self, message: Message) -> ClientResult<()> {
        let sender = self.message_sender.as_ref()
            .ok_or_else(|| ClientError::Connection("Not connected".to_string()))?;
        
        sender.send(message)
            .map_err(|_| ClientError::Connection("Connection closed".to_string()))?;
        
        // Update send stats
        {
            let mut stats = self.stats.write().await;
            stats.messages_sent += 1;
        }
        
        Ok(())
    }
    
    /// Check if connected
    pub async fn is_connected(&self) -> bool {
        matches!(*self.state.read().await, ConnectionState::Connected)
    }
    
    /// Get current connection state
    pub async fn state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }
    
    /// Get connection statistics
    pub async fn stats(&self) -> ConnectionStats {
        self.stats.read().await.clone()
    }
    
    /// Get agent configuration
    pub fn agent_config(&self) -> &AgentConfig {
        &self.config
    }
    
    /// Set connection state
    async fn set_state(&self, new_state: ConnectionState) {
        let mut state = self.state.write().await;
        if *state != new_state {
            debug!("Agent {} state changed: {:?} -> {:?}", self.config.id, *state, new_state);
            *state = new_state;
        }
    }
    
    /// Establish the WebSocket connection and start background tasks
    async fn establish_connection(&self) -> ClientResult<(
        mpsc::UnboundedSender<Message>,
        oneshot::Sender<()>,
        Vec<tokio::task::JoinHandle<()>>
    )> {
        // Connect to WebSocket
        let url = url::Url::parse(&self.config.url)?;
        let (ws_stream, _) = timeout(
            self.connection_config.connection_timeout(),
            connect_async(url)
        ).await
        .map_err(|_| ClientError::Timeout { 
            seconds: self.connection_config.connect_timeout_ms / 1000 
        })?
        .map_err(|e| ClientError::Network(e))?;
        
        let (ws_sink, ws_stream) = ws_stream.split();
        
        // Create channels
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        
        // Start background tasks
        let mut tasks = Vec::new();
        
        // Clone the necessary components for the tasks
        let agent_id = self.config.id.clone();
        let stats = self.stats.clone();
        let state = self.state.clone();
        let pending_requests = self.pending_requests.clone();
        let heartbeat_interval_ms = self.connection_config.heartbeat_interval_ms;
        
        // Message sender task
        tasks.push(tokio::spawn(
            Self::message_sender_task(
                agent_id.clone(),
                stats.clone(),
                ws_sink,
                message_rx,
                shutdown_rx,
            )
        ));
        
        // Message receiver task  
        tasks.push(tokio::spawn(
            Self::message_receiver_task(
                agent_id.clone(),
                state,
                stats,
                pending_requests,
                ws_stream,
            )
        ));
        
        // Heartbeat task
        if heartbeat_interval_ms > 0 {
            tasks.push(tokio::spawn(
                Self::heartbeat_task(
                    agent_id,
                    heartbeat_interval_ms,
                    message_tx.clone(),
                )
            ));
        }
        
        Ok((message_tx, shutdown_tx, tasks))
    }
    
    /// Task for sending messages to WebSocket
    async fn message_sender_task(
        agent_id: String,
        stats: Arc<RwLock<ConnectionStats>>,
        mut ws_sink: futures::stream::SplitSink<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, WsMessage>,
        mut message_rx: mpsc::UnboundedReceiver<Message>,
        mut shutdown_rx: oneshot::Receiver<()>,
    ) {
        loop {
            tokio::select! {
                message = message_rx.recv() => {
                    match message {
                        Some(msg) => {
                            match bincode::serialize(&msg) {
                                Ok(data) => {
                                    let data_len = data.len();
                                    let ws_msg = WsMessage::Binary(data);
                                    if let Err(e) = ws_sink.send(ws_msg).await {
                                        error!("Failed to send message to agent {}: {}", agent_id, e);
                                        break;
                                    }
                                    
                                    // Update send stats
                                    {
                                        let mut stats_guard = stats.write().await;
                                        stats_guard.bytes_sent += data_len as u64;
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to serialize message for agent {}: {}", agent_id, e);
                                }
                            }
                        }
                        None => break,
                    }
                }
                _ = &mut shutdown_rx => {
                    debug!("Shutting down message sender for agent {}", agent_id);
                    break;
                }
            }
        }
        
        let _ = ws_sink.close().await;
    }
    
    /// Task for receiving messages from WebSocket
    async fn message_receiver_task(
        agent_id: String,
        state: Arc<RwLock<ConnectionState>>,
        stats: Arc<RwLock<ConnectionStats>>,
        pending_requests: Arc<DashMap<Uuid, ResponseWaiter>>,
        mut ws_stream: futures::stream::SplitStream<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>,
    ) {
        while let Some(ws_msg) = ws_stream.next().await {
            match ws_msg {
                Ok(WsMessage::Binary(data)) => {
                    match bincode::deserialize::<Message>(&data) {
                        Ok(message) => {
                            // Update receive stats
                            {
                                let mut stats_guard = stats.write().await;
                                stats_guard.messages_received += 1;
                                stats_guard.bytes_received += data.len() as u64;
                            }
                            
                            Self::handle_received_message_static(
                                agent_id.clone(),
                                pending_requests.clone(),
                                message
                            ).await;
                        }
                        Err(e) => {
                            warn!("Failed to deserialize message from agent {}: {}", agent_id, e);
                        }
                    }
                }
                Ok(WsMessage::Close(_)) => {
                    info!("WebSocket connection closed by agent {}", agent_id);
                    break;
                }
                Ok(WsMessage::Ping(_data)) => {
                    debug!("Received ping from agent {}", agent_id);
                    // Pong is automatically handled by tungstenite
                }
                Ok(WsMessage::Pong(_)) => {
                    debug!("Received pong from agent {}", agent_id);
                }
                Ok(_) => {
                    // Ignore other message types
                }
                Err(e) => {
                    error!("WebSocket error from agent {}: {}", agent_id, e);
                    break;
                }
            }
        }
        
        // Connection lost
        let mut state_guard = state.write().await;
        if *state_guard != ConnectionState::Disconnected {
            debug!("Agent {} state changed: {:?} -> {:?}", agent_id, *state_guard, ConnectionState::Disconnected);
            *state_guard = ConnectionState::Disconnected;
        }
    }
    
    /// Static version of handle_received_message for use in spawned tasks
    async fn handle_received_message_static(
        agent_id: String,
        pending_requests: Arc<DashMap<Uuid, ResponseWaiter>>,
        message: Message,
    ) {
        let request_id = message.request_id();
        
        // Check if this is a response to a pending request
        if let Some(request_id) = request_id {
            if let Some((_, response_tx)) = pending_requests.remove(&request_id) {
                let _ = response_tx.send(Ok(message));
            }
        } else {
            // This is an unsolicited message (notification, event, etc.)
            debug!("Received unsolicited message from agent {}: {:?}", agent_id, message);
        }
    }
    
    /// Heartbeat task to keep connection alive
    async fn heartbeat_task(
        agent_id: String,
        heartbeat_interval_ms: u64,
        message_tx: mpsc::UnboundedSender<Message>,
    ) {
        let mut interval = tokio::time::interval(
            Duration::from_millis(heartbeat_interval_ms)
        );
        
        loop {
            interval.tick().await;
            
            let heartbeat = Message::Ping {
                timestamp: chrono::Utc::now(),
            };
            
            if message_tx.send(heartbeat).is_err() {
                debug!("Heartbeat channel closed for agent {}", agent_id);
                break;
            }
        }
    }
}

impl Drop for AgentConnection {
    fn drop(&mut self) {
        // Send shutdown signal if still connected
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

/// Connection pool for managing multiple agent connections
pub struct ConnectionPool {
    connections: Arc<RwLock<Vec<Arc<Mutex<AgentConnection>>>>>,
    connection_config: ConnectionConfig,
    load_balancer: Arc<AtomicU64>,
}

impl ConnectionPool {
    /// Create a new connection pool
    pub fn new(connection_config: ConnectionConfig) -> Self {
        Self {
            connections: Arc::new(RwLock::new(Vec::new())),
            connection_config,
            load_balancer: Arc::new(AtomicU64::new(0)),
        }
    }
    
    /// Add an agent to the pool
    pub async fn add_agent(&self, agent_config: AgentConfig) {
        let connection = Arc::new(Mutex::new(
            AgentConnection::new(agent_config, self.connection_config.clone())
        ));
        
        self.connections.write().await.push(connection);
    }
    
    /// Get the next available connection using load balancing
    pub async fn get_connection(&self) -> ClientResult<Arc<Mutex<AgentConnection>>> {
        let connections = self.connections.read().await;
        
        if connections.is_empty() {
            return Err(ClientError::Configuration(
                "No agents configured".to_string()
            ));
        }
        
        // Simple round-robin for now
        let index = self.load_balancer.fetch_add(1, Ordering::Relaxed) as usize % connections.len();
        let connection = connections[index].clone();
        
        // Check if connection is healthy, try to connect if not
        {
            let mut conn = connection.lock().await;
            if !conn.is_connected().await {
                conn.connect().await?;
            }
        }
        
        Ok(connection)
    }
    
    /// Get all connections
    pub async fn get_all_connections(&self) -> Vec<Arc<Mutex<AgentConnection>>> {
        self.connections.read().await.clone()
    }
    
    /// Connect all agents
    pub async fn connect_all(&self) -> Vec<ClientResult<()>> {
        let connections = self.connections.read().await.clone();
        let mut results = Vec::new();
        
        for connection in connections {
            let mut conn = connection.lock().await;
            results.push(conn.connect().await);
        }
        
        results
    }
    
    /// Disconnect all agents
    pub async fn disconnect_all(&self) -> Vec<ClientResult<()>> {
        let connections = self.connections.read().await.clone();
        let mut results = Vec::new();
        
        for connection in connections {
            let mut conn = connection.lock().await;
            results.push(conn.disconnect().await);
        }
        
        results
    }
}
