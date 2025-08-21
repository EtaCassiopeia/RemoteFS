use axum::extract::ws::Message as WsMessage;
use remotefs_common::{
    protocol::{NodeType, RelayInfo},
    config::RelayConfig,
    error::{RemoteFsError, Result},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, warn};
use uuid::Uuid;

/// Represents an active session with a client or agent
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub node_id: String,
    pub node_type: NodeType,
    pub connection_id: Uuid,
    pub created_at: u64,
    pub last_activity: Arc<RwLock<u64>>,
    pub sender: mpsc::UnboundedSender<WsMessage>,
    pub message_format: MessageFormat,
}

/// Message format preference for the session
#[derive(Debug, Clone, Copy)]
pub enum MessageFormat {
    Json,
    Binary,
}

impl Session {
    /// Create a new session
    pub fn new(
        id: String,
        node_id: String,
        node_type: NodeType,
        connection_id: Uuid,
        sender: mpsc::UnboundedSender<WsMessage>,
        message_format: MessageFormat,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        Self {
            id,
            node_id,
            node_type,
            connection_id,
            created_at: now,
            last_activity: Arc::new(RwLock::new(now)),
            sender,
            message_format,
        }
    }
    
    /// Update the last activity timestamp
    pub async fn update_activity(&self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        *self.last_activity.write().await = now;
    }
    
    /// Check if the session is expired
    pub async fn is_expired(&self, timeout_seconds: u64) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let last_activity = *self.last_activity.read().await;
        now.saturating_sub(last_activity) > timeout_seconds
    }
    
    /// Send a message to this session
    pub async fn send_message(&self, message: WsMessage) -> Result<()> {
        self.sender.send(message)
            .map_err(|_| RemoteFsError::Network("Failed to send message to session".to_string()))?;
        
        self.update_activity().await;
        Ok(())
    }
}

/// Statistics about sessions
#[derive(Debug, Clone)]
pub struct SessionStats {
    pub active_sessions: usize,
    pub total_clients: usize,
    pub total_agents: usize,
}

/// Manages all active sessions
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, Session>>>,
    config: RelayConfig,
}

impl SessionManager {
    /// Create a new session manager
    pub fn new(config: &RelayConfig) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config: config.clone(),
        }
    }
    
    /// Add a new session
    pub async fn add_session(&self, session: Session) {
        debug!("Adding session: {} for node: {}", session.id, session.node_id);
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session);
    }
    
    /// Remove a session
    pub async fn remove_session(&self, session_id: &str) -> Option<Session> {
        debug!("Removing session: {}", session_id);
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id)
    }
    
    /// Get a session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }
    
    /// Get a session by node ID
    pub async fn get_session_by_node(&self, node_id: &str) -> Option<Session> {
        let sessions = self.sessions.read().await;
        sessions.values()
            .find(|session| session.node_id == node_id)
            .cloned()
    }
    
    /// Get all sessions of a specific type
    pub async fn get_sessions_by_type(&self, node_type: NodeType) -> Vec<Session> {
        let sessions = self.sessions.read().await;
        sessions.values()
            .filter(|session| std::mem::discriminant(&session.node_type) == std::mem::discriminant(&node_type))
            .cloned()
            .collect()
    }
    
    /// Update activity for a session
    pub async fn update_session_activity(&self, session_id: &str) -> Result<()> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            session.update_activity().await;
            Ok(())
        } else {
            Err(RemoteFsError::NotFound(format!("Session not found: {}", session_id)))
        }
    }
    
    /// Clean up expired sessions
    pub async fn cleanup_expired_sessions(&self) -> usize {
        let timeout = self.config.session.timeout;
        let mut sessions_to_remove = Vec::new();
        
        // First, identify expired sessions
        {
            let sessions = self.sessions.read().await;
            for (session_id, session) in sessions.iter() {
                if session.is_expired(timeout).await {
                    debug!("Session expired: {} (node: {})", session_id, session.node_id);
                    sessions_to_remove.push(session_id.clone());
                }
            }
        }
        
        // Then remove them
        let expired_count = sessions_to_remove.len();
        if expired_count > 0 {
            let mut sessions = self.sessions.write().await;
            for session_id in sessions_to_remove {
                sessions.remove(&session_id);
            }
        }
        
        expired_count
    }
    
    /// Get session statistics
    pub async fn get_stats(&self) -> SessionStats {
        let sessions = self.sessions.read().await;
        let active_sessions = sessions.len();
        
        let mut total_clients = 0;
        let mut total_agents = 0;
        
        for session in sessions.values() {
            match session.node_type {
                NodeType::Client => total_clients += 1,
                NodeType::Agent => total_agents += 1,
                NodeType::Relay => {} // Relays don't connect to other relays in this design
            }
        }
        
        SessionStats {
            active_sessions,
            total_clients,
            total_agents,
        }
    }
    
    /// Get relay information for auth responses
    pub fn get_relay_info(&self) -> RelayInfo {
        RelayInfo {
            relay_id: "relay-001".to_string(), // TODO: Make configurable
            capabilities: vec![
                "routing".to_string(),
                "authentication".to_string(),
                "session_management".to_string(),
            ],
            max_message_size: self.config.message_limits.max_message_size as u64,
            heartbeat_interval: self.config.network.heartbeat_interval,
        }
    }
    
    /// Send a message to a specific session
    pub async fn send_to_session(&self, session_id: &str, message: WsMessage) -> Result<()> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            session.send_message(message).await
        } else {
            Err(RemoteFsError::NotFound(format!("Session not found: {}", session_id)))
        }
    }
    
    /// Send a message to a node by node ID
    pub async fn send_to_node(&self, node_id: &str, message: WsMessage) -> Result<()> {
        let sessions = self.sessions.read().await;
        if let Some(session) = sessions.values().find(|s| s.node_id == node_id) {
            session.send_message(message).await
        } else {
            Err(RemoteFsError::NotFound(format!("Node not found: {}", node_id)))
        }
    }
    
    /// Broadcast a message to all sessions of a specific type
    pub async fn broadcast_to_type(&self, node_type: NodeType, message: WsMessage) -> Vec<String> {
        let sessions = self.sessions.read().await;
        let mut failed_sessions = Vec::new();
        
        for session in sessions.values() {
            if std::mem::discriminant(&session.node_type) == std::mem::discriminant(&node_type) {
                if let Err(e) = session.send_message(message.clone()).await {
                    warn!("Failed to send message to session {}: {}", session.id, e);
                    failed_sessions.push(session.id.clone());
                }
            }
        }
        
        failed_sessions
    }
    
    /// Get all active node IDs by type
    pub async fn get_active_nodes(&self, node_type: NodeType) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.values()
            .filter(|session| std::mem::discriminant(&session.node_type) == std::mem::discriminant(&node_type))
            .map(|session| session.node_id.clone())
            .collect()
    }
    
    /// Check if a node is currently connected
    pub async fn is_node_connected(&self, node_id: &str) -> bool {
        let sessions = self.sessions.read().await;
        sessions.values().any(|session| session.node_id == node_id)
    }
    
    /// Get session count by type
    pub async fn count_sessions_by_type(&self, node_type: NodeType) -> usize {
        let sessions = self.sessions.read().await;
        sessions.values()
            .filter(|session| std::mem::discriminant(&session.node_type) == std::mem::discriminant(&node_type))
            .count()
    }
    
    /// Force disconnect a session
    pub async fn disconnect_session(&self, session_id: &str) -> Result<()> {
        if let Some(session) = self.remove_session(session_id).await {
            // Send a close message to trigger disconnection
            let close_msg = WsMessage::Close(Some(axum::extract::ws::CloseFrame {
                code: axum::extract::ws::close_code::NORMAL,
                reason: "Server initiated disconnect".into(),
            }));
            
            let _ = session.send_message(close_msg).await;
            debug!("Disconnected session: {} (node: {})", session_id, session.node_id);
            Ok(())
        } else {
            Err(RemoteFsError::NotFound(format!("Session not found: {}", session_id)))
        }
    }
    
    /// Force disconnect a node
    pub async fn disconnect_node(&self, node_id: &str) -> Result<()> {
        let session_id = {
            let sessions = self.sessions.read().await;
            sessions.values()
                .find(|session| session.node_id == node_id)
                .map(|session| session.id.clone())
        };
        
        if let Some(session_id) = session_id {
            self.disconnect_session(&session_id).await
        } else {
            Err(RemoteFsError::NotFound(format!("Node not found: {}", node_id)))
        }
    }
}

/// Convert from server.rs MessageFormat to session.rs MessageFormat
impl From<crate::server::MessageFormat> for MessageFormat {
    fn from(format: crate::server::MessageFormat) -> Self {
        match format {
            crate::server::MessageFormat::Json => MessageFormat::Json,
            crate::server::MessageFormat::Binary => MessageFormat::Binary,
        }
    }
}

/// Convert from session.rs MessageFormat to server.rs MessageFormat  
impl From<MessageFormat> for crate::server::MessageFormat {
    fn from(format: MessageFormat) -> Self {
        match format {
            MessageFormat::Json => crate::server::MessageFormat::Json,
            MessageFormat::Binary => crate::server::MessageFormat::Binary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use remotefs_common::config_utils;
    
    #[tokio::test]
    async fn test_session_management() {
        let config = config_utils::create_default_relay_config();
        let manager = SessionManager::new(&config);
        
        // Create a test session
        let (tx, _rx) = mpsc::unbounded_channel();
        let session = Session::new(
            "test-session".to_string(),
            "test-node".to_string(),
            NodeType::Client,
            Uuid::new_v4(),
            tx,
            MessageFormat::Json,
        );
        
        // Test adding session
        manager.add_session(session.clone()).await;
        
        // Test retrieving session
        let retrieved = manager.get_session("test-session").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().node_id, "test-node");
        
        // Test getting session by node
        let by_node = manager.get_session_by_node("test-node").await;
        assert!(by_node.is_some());
        
        // Test stats
        let stats = manager.get_stats().await;
        assert_eq!(stats.active_sessions, 1);
        assert_eq!(stats.total_clients, 1);
        assert_eq!(stats.total_agents, 0);
        
        // Test removing session
        let removed = manager.remove_session("test-session").await;
        assert!(removed.is_some());
        
        let stats = manager.get_stats().await;
        assert_eq!(stats.active_sessions, 0);
    }
    
    #[tokio::test]
    async fn test_session_expiry() {
        let mut config = config_utils::create_default_relay_config();
        config.session.timeout = 1; // 1 second timeout
        
        let manager = SessionManager::new(&config);
        let (tx, _rx) = mpsc::unbounded_channel();
        
        let session = Session::new(
            "expire-test".to_string(),
            "expire-node".to_string(),
            NodeType::Agent,
            Uuid::new_v4(),
            tx,
            MessageFormat::Binary,
        );
        
        // Test that new session is not expired
        assert!(!session.is_expired(1).await);
        
        manager.add_session(session).await;
        
        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        // Test cleanup
        let expired_count = manager.cleanup_expired_sessions().await;
        assert_eq!(expired_count, 1);
        
        // Verify session is removed
        let stats = manager.get_stats().await;
        assert_eq!(stats.active_sessions, 0);
    }
}
