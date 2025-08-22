use crate::session::{Session, SessionManager};
use crate::routing::MessageRouter;
use crate::auth::AuthManager;
use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        State,
    },
    response::Response,
    routing::get,
    Router,
};
use remotefs_common::{
    protocol::{Message, NodeType, generate_request_id},
    error::{RemoteFsError, Result},
    config::RelayConfig,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn, error, debug};
use uuid::Uuid;

/// Main relay server that handles client and agent connections
pub struct RelayServer {
    config: RelayConfig,
    session_manager: Arc<SessionManager>,
    message_router: Arc<MessageRouter>,
    auth_manager: Arc<AuthManager>,
    shutdown_tx: broadcast::Sender<()>,
    shutdown_rx: broadcast::Receiver<()>,
}

impl RelayServer {
    /// Create a new relay server with the given configuration and auth manager
    pub fn new(config: RelayConfig, auth_manager: Arc<AuthManager>) -> Result<Self> {
        let (shutdown_tx, shutdown_rx) = broadcast::channel(1);
        
        Ok(Self {
            session_manager: Arc::new(SessionManager::new(&config)),
            message_router: Arc::new(MessageRouter::new()),
            auth_manager,
            config,
            shutdown_tx,
            shutdown_rx,
        })
    }
    
    /// Start the relay server
    pub async fn run(&self) -> Result<()> {
        let addr = SocketAddr::new(
            self.config.bind_address.parse()
                .map_err(|e| RemoteFsError::Configuration(format!("Invalid bind address: {}", e)))?,
            self.config.port,
        );
        
        info!("Starting RemoteFS relay server on {}", addr);
        
        // Create the application state
        let app_state = AppState {
            session_manager: Arc::clone(&self.session_manager),
            message_router: Arc::clone(&self.message_router),
            auth_manager: Arc::clone(&self.auth_manager),
            config: self.config.clone(),
        };
        
        // Create the router
        let app = Router::new()
            .route("/ws", get(websocket_handler))
            .route("/health", get(health_handler))
            .route("/stats", get(stats_handler))
            .with_state(app_state);
        
        // Start the server
        let listener = tokio::net::TcpListener::bind(addr).await
            .map_err(|e| RemoteFsError::Network(format!("Failed to bind to {}: {}", addr, e)))?;
            
        info!("Relay server listening on {}", addr);
        
        // Start background tasks
        let session_cleanup = self.start_session_cleanup();
        let stats_reporter = self.start_stats_reporter();
        
        // Run the server
        let server = axum::serve(listener, app);
        
        tokio::select! {
            result = server => {
                if let Err(e) = result {
                    error!("Server error: {}", e);
                    return Err(RemoteFsError::Network(format!("Server error: {}", e)));
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Received shutdown signal");
            }
            _ = session_cleanup => {
                warn!("Session cleanup task ended unexpectedly");
            }
            _ = stats_reporter => {
                warn!("Stats reporter task ended unexpectedly");
            }
        }
        
        info!("Shutting down relay server");
        let _ = self.shutdown_tx.send(());
        
        Ok(())
    }
    
    /// Start the session cleanup background task
    fn start_session_cleanup(&self) -> tokio::task::JoinHandle<()> {
        let session_manager = Arc::clone(&self.session_manager);
        let cleanup_interval = self.config.session.cleanup_interval;
        let mut shutdown_rx = self.shutdown_rx.resubscribe();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(cleanup_interval)
            );
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let expired = session_manager.cleanup_expired_sessions().await;
                        if expired > 0 {
                            debug!("Cleaned up {} expired sessions", expired);
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Session cleanup task shutting down");
                        break;
                    }
                }
            }
        })
    }
    
    /// Start the stats reporting background task
    fn start_stats_reporter(&self) -> tokio::task::JoinHandle<()> {
        let session_manager = Arc::clone(&self.session_manager);
        let message_router = Arc::clone(&self.message_router);
        let mut shutdown_rx = self.shutdown_rx.resubscribe();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(
                tokio::time::Duration::from_secs(300) // Report every 5 minutes
            );
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let session_stats = session_manager.get_stats().await;
                        let routing_stats = message_router.get_stats().await;
                        
                        info!("Relay Server Stats:");
                        info!("  Active sessions: {}", session_stats.active_sessions);
                        info!("  Total clients: {}", session_stats.total_clients);
                        info!("  Total agents: {}", session_stats.total_agents);
                        info!("  Messages routed: {}", routing_stats.messages_routed);
                        info!("  Failed routes: {}", routing_stats.failed_routes);
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Stats reporter task shutting down");
                        break;
                    }
                }
            }
        })
    }
}

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    pub session_manager: Arc<SessionManager>,
    pub message_router: Arc<MessageRouter>,
    pub auth_manager: Arc<AuthManager>,
    pub config: RelayConfig,
}

/// WebSocket upgrade handler
pub async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(|socket| handle_websocket(socket, state))
}

/// Health check handler
pub async fn health_handler() -> &'static str {
    "OK"
}

/// Stats handler
pub async fn stats_handler(State(state): State<AppState>) -> String {
    let session_stats = state.session_manager.get_stats().await;
    let routing_stats = state.message_router.get_stats().await;
    
    format!(
        "RemoteFS Relay Server Stats\n\
         Active Sessions: {}\n\
         Total Clients: {}\n\
         Total Agents: {}\n\
         Messages Routed: {}\n\
         Failed Routes: {}\n\
         Uptime: {}",
        session_stats.active_sessions,
        session_stats.total_clients,
        session_stats.total_agents,
        routing_stats.messages_routed,
        routing_stats.failed_routes,
        "N/A" // TODO: Add uptime tracking
    )
}

/// Handle individual WebSocket connections
async fn handle_websocket(socket: WebSocket, state: AppState) {
    let connection_id = Uuid::new_v4();
    debug!("New WebSocket connection: {}", connection_id);
    
    let (mut sender, mut receiver) = socket.split();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    
    // Spawn task to handle outgoing messages
    let sender_task = tokio::spawn(async move {
        let mut rx = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
        while let Some(msg) = futures::stream::StreamExt::next(&mut rx).await {
            if sender.send(msg).await.is_err() {
                break;
            }
        }
    });
    
    // Handle incoming messages
    let mut session: Option<Session> = None;
    
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(WsMessage::Text(text)) => {
                match handle_text_message(&text, &mut session, &state, &tx, connection_id).await {
                    Ok(()) => {}
                    Err(e) => {
                        warn!("Error handling text message: {}", e);
                        let error_msg = create_error_message(None, e);
                        if let Ok(response) = serde_json::to_string(&error_msg) {
                            let _ = tx.send(WsMessage::Text(response));
                        }
                    }
                }
            }
            Ok(WsMessage::Binary(data)) => {
                match handle_binary_message(&data, &mut session, &state, &tx, connection_id).await {
                    Ok(()) => {}
                    Err(e) => {
                        warn!("Error handling binary message: {}", e);
                        let error_msg = create_error_message(None, e);
                        if let Ok(response) = bincode::serialize(&error_msg) {
                            let _ = tx.send(WsMessage::Binary(response));
                        }
                    }
                }
            }
            Ok(WsMessage::Close(_)) => {
                debug!("WebSocket connection closed: {}", connection_id);
                break;
            }
            Ok(WsMessage::Ping(data)) => {
                let _ = tx.send(WsMessage::Pong(data));
            }
            Ok(WsMessage::Pong(_)) => {
                // Handle pong if needed
            }
            Err(e) => {
                warn!("WebSocket error: {}", e);
                break;
            }
        }
    }
    
    // Clean up session if it exists
    if let Some(session) = session {
        state.session_manager.remove_session(&session.id).await;
        debug!("Removed session: {}", session.id);
    }
    
    sender_task.abort();
    debug!("WebSocket connection ended: {}", connection_id);
}

/// Handle text messages (JSON)
async fn handle_text_message(
    text: &str,
    session: &mut Option<Session>,
    state: &AppState,
    tx: &tokio::sync::mpsc::UnboundedSender<WsMessage>,
    connection_id: Uuid,
) -> Result<()> {
    let message: Message = serde_json::from_str(text)
        .map_err(|e| RemoteFsError::Protocol(format!("Invalid JSON message: {}", e)))?;
    
    handle_message(message, session, state, tx, connection_id, MessageFormat::Json).await
}

/// Handle binary messages (bincode)  
async fn handle_binary_message(
    data: &[u8],
    session: &mut Option<Session>,
    state: &AppState,
    tx: &tokio::sync::mpsc::UnboundedSender<WsMessage>,
    connection_id: Uuid,
) -> Result<()> {
    let message: Message = bincode::deserialize(data)
        .map_err(|e| RemoteFsError::Protocol(format!("Invalid binary message: {}", e)))?;
    
    handle_message(message, session, state, tx, connection_id, MessageFormat::Binary).await
}

/// Message format for responses
#[derive(Clone, Copy)]
pub enum MessageFormat {
    Json,
    Binary,
}

/// Handle a parsed message
async fn handle_message(
    message: Message,
    session: &mut Option<Session>,
    state: &AppState,
    tx: &tokio::sync::mpsc::UnboundedSender<WsMessage>,
    connection_id: Uuid,
    format: MessageFormat,
) -> Result<()> {
    debug!("Handling message: {} from connection: {}", message.message_type(), connection_id);
    
    match message {
        Message::AuthRequest { node_id, node_type, public_key, capabilities } => {
            handle_auth_request(
                node_id, node_type, public_key, capabilities,
                session, state, tx, connection_id, format
            ).await
        }
        
        Message::EstablishChannel { target_node, encrypted_key_exchange } => {
            handle_establish_channel(
                target_node, encrypted_key_exchange,
                session, state, tx, format
            ).await
        }
        
        Message::Ping { timestamp } => {
            handle_ping(timestamp, tx, format).await
        }
        
        // All other messages are routed between clients and agents
        _ => {
            if let Some(session) = session {
                state.message_router.route_message(message, session, state).await?;
            } else {
                return Err(RemoteFsError::Authentication("No active session".to_string()));
            }
            Ok(())
        }
    }
}

/// Handle authentication requests
async fn handle_auth_request(
    node_id: String,
    node_type: NodeType,
    public_key: Vec<u8>,
    capabilities: Vec<String>,
    session: &mut Option<Session>,
    state: &AppState,
    tx: &tokio::sync::mpsc::UnboundedSender<WsMessage>,
    connection_id: Uuid,
    format: MessageFormat,
) -> Result<()> {
    debug!("Authentication request from {} ({})", node_id, match node_type {
        NodeType::Client => "client",
        NodeType::Agent => "agent", 
        NodeType::Relay => "relay",
    });
    
    // Authenticate the node
    let auth_result = state.auth_manager.authenticate_node(
        &node_id, &node_type, &public_key, &capabilities
    ).await;
    
    let response = match auth_result {
        Ok(session_token) => {
            // Create session
            let new_session = Session::new(
                generate_request_id().to_string(),
                node_id.clone(),
                node_type,
                connection_id,
                tx.clone(),
                format.into(),
            );
            
            // Store session
            state.session_manager.add_session(new_session.clone()).await;
            *session = Some(new_session);
            
            Message::AuthResponse {
                success: true,
                session_token: Some(session_token),
                relay_info: Some(state.session_manager.get_relay_info()),
                error: None,
            }
        }
        Err(e) => {
            warn!("Authentication failed for {}: {}", node_id, e);
            Message::AuthResponse {
                success: false,
                session_token: None,
                relay_info: None,
                error: Some(e.to_string()),
            }
        }
    };
    
    send_message(response, tx, format).await
}

/// Handle channel establishment requests
async fn handle_establish_channel(
    target_node: String,
    encrypted_key_exchange: Vec<u8>,
    session: &mut Option<Session>,
    state: &AppState,
    tx: &tokio::sync::mpsc::UnboundedSender<WsMessage>,
    format: MessageFormat,
) -> Result<()> {
    if let Some(_session) = session {
        // Forward the channel establishment request to the target node
        let establish_message = Message::EstablishChannel {
            target_node: target_node.clone(),
            encrypted_key_exchange,
        };
        
        match state.message_router.route_to_node(establish_message, &target_node).await {
            Ok(()) => {
                debug!("Channel establishment request forwarded to {}", target_node);
            }
            Err(e) => {
                warn!("Failed to forward channel establishment to {}: {}", target_node, e);
                let response = Message::ChannelEstablished {
                    success: false,
                    encrypted_response: None,
                    error: Some(format!("Target node not found: {}", target_node)),
                };
                send_message(response, tx, format).await?;
            }
        }
    } else {
        return Err(RemoteFsError::Authentication("No active session".to_string()));
    }
    
    Ok(())
}

/// Handle ping messages
async fn handle_ping(
    original_timestamp: chrono::DateTime<chrono::Utc>,
    tx: &tokio::sync::mpsc::UnboundedSender<WsMessage>,
    format: MessageFormat,
) -> Result<()> {
    let response = Message::Pong {
        timestamp: chrono::Utc::now(),
        original_timestamp,
    };
    
    send_message(response, tx, format).await
}

/// Send a message through the WebSocket
async fn send_message(
    message: Message,
    tx: &tokio::sync::mpsc::UnboundedSender<WsMessage>,
    format: MessageFormat,
) -> Result<()> {
    let ws_message = match format {
        MessageFormat::Json => {
            let json = serde_json::to_string(&message)
                .map_err(|e| RemoteFsError::Protocol(format!("JSON serialization error: {}", e)))?;
            WsMessage::Text(json)
        }
        MessageFormat::Binary => {
            let binary = bincode::serialize(&message)
                .map_err(|e| RemoteFsError::Protocol(format!("Binary serialization error: {}", e)))?;
            WsMessage::Binary(binary)
        }
    };
    
    tx.send(ws_message)
        .map_err(|_| RemoteFsError::Network("Failed to send message".to_string()))?;
    
    Ok(())
}

/// Create an error message
fn create_error_message(request_id: Option<uuid::Uuid>, error: RemoteFsError) -> Message {
    Message::Error {
        request_id,
        code: error.to_error_code(),
        message: error.to_string(),
        details: None,
    }
}

use futures::stream::StreamExt;
use futures::SinkExt;
