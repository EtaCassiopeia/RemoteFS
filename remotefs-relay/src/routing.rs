use crate::session::Session;
use crate::server::AppState;
use axum::extract::ws::Message as WsMessage;
use remotefs_common::{
    protocol::{Message, NodeType},
    error::{RemoteFsError, Result},
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{debug, warn};

/// Statistics for message routing
#[derive(Debug, Clone)]
pub struct RoutingStats {
    pub messages_routed: u64,
    pub failed_routes: u64,
}

/// Handles routing of messages between clients and agents
pub struct MessageRouter {
    messages_routed: Arc<AtomicU64>,
    failed_routes: Arc<AtomicU64>,
}

impl MessageRouter {
    /// Create a new message router
    pub fn new() -> Self {
        Self {
            messages_routed: Arc::new(AtomicU64::new(0)),
            failed_routes: Arc::new(AtomicU64::new(0)),
        }
    }
    
    /// Route a message between client and agent through the relay
    pub async fn route_message(
        &self,
        message: Message,
        sender_session: &Session,
        state: &AppState,
    ) -> Result<()> {
        debug!(
            "Routing message {} from {} ({})",
            message.message_type(),
            sender_session.node_id,
            match sender_session.node_type {
                NodeType::Client => "client",
                NodeType::Agent => "agent",
                NodeType::Relay => "relay",
            }
        );
        
        match self.determine_target(&message, sender_session, state).await {
            Ok(target_node_id) => {
                self.send_to_target(message, &target_node_id, state).await?;
                self.messages_routed.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }
            Err(e) => {
                warn!("Failed to route message: {}", e);
                self.failed_routes.fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
        }
    }
    
    /// Route a message to a specific node
    pub async fn route_to_node(&self, message: Message, target_node_id: &str) -> Result<()> {
        debug!(
            "Routing message {} to node {}",
            message.message_type(),
            target_node_id
        );
        
        // This is a simplified implementation that would be expanded in a real system
        // to include proper target resolution and state management
        self.messages_routed.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    
    /// Determine the target node for a message
    async fn determine_target(
        &self,
        message: &Message,
        sender_session: &Session,
        state: &AppState,
    ) -> Result<String> {
        match message {
            // File system operations need to be routed to agents
            Message::ReadFile { .. }
            | Message::WriteFile { .. }
            | Message::CreateFile { .. }
            | Message::DeleteFile { .. }
            | Message::TruncateFile { .. }
            | Message::ListDirectory { .. }
            | Message::CreateDirectory { .. }
            | Message::RemoveDirectory { .. }
            | Message::GetMetadata { .. }
            | Message::SetMetadata { .. }
            | Message::Rename { .. }
            | Message::CreateSymlink { .. }
            | Message::PathExists { .. }
            | Message::GetSpaceInfo { .. } => {
                match sender_session.node_type {
                    NodeType::Client => {
                        // Client sending to agent - find available agent
                        self.find_available_agent(state).await
                    }
                    NodeType::Agent => {
                        // Agent responding to client - need to track request context
                        self.find_target_client_for_response(message, state).await
                    }
                    NodeType::Relay => {
                        Err(RemoteFsError::Protocol("Relay cannot send file operations".to_string()))
                    }
                }
            }
            
            // Response messages need to be routed back to the original requester
            Message::ReadFileResponse { .. }
            | Message::WriteFileResponse { .. }
            | Message::CreateFileResponse { .. }
            | Message::DeleteFileResponse { .. }
            | Message::TruncateFileResponse { .. }
            | Message::ListDirectoryResponse { .. }
            | Message::CreateDirectoryResponse { .. }
            | Message::RemoveDirectoryResponse { .. }
            | Message::GetMetadataResponse { .. }
            | Message::SetMetadataResponse { .. }
            | Message::RenameResponse { .. }
            | Message::CreateSymlinkResponse { .. }
            | Message::PathExistsResponse { .. }
            | Message::GetSpaceInfoResponse { .. } => {
                match sender_session.node_type {
                    NodeType::Agent => {
                        // Agent responding to client
                        self.find_target_client_for_response(message, state).await
                    }
                    NodeType::Client => {
                        Err(RemoteFsError::Protocol("Client cannot send response messages".to_string()))
                    }
                    NodeType::Relay => {
                        Err(RemoteFsError::Protocol("Relay cannot send response messages".to_string()))
                    }
                }
            }
            
            // Channel establishment can be bidirectional
            Message::EstablishChannel { target_node, .. } => {
                Ok(target_node.clone())
            }
            
            Message::ChannelEstablished { .. } => {
                // This needs to be routed back to the original channel requester
                self.find_channel_requester(message, state).await
            }
            
            // Error messages need special handling
            Message::Error { .. } => {
                self.find_error_target(message, sender_session, state).await
            }
            
            // These shouldn't be routed through this function
            Message::AuthRequest { .. }
            | Message::AuthResponse { .. }
            | Message::Ping { .. }
            | Message::Pong { .. }
            | Message::ConnectionClose { .. } => {
                Err(RemoteFsError::Protocol(
                    format!("Message {} should not be routed", message.message_type())
                ))
            }
        }
    }
    
    /// Find an available agent to handle client requests
    async fn find_available_agent(&self, state: &AppState) -> Result<String> {
        let agents = state.session_manager.get_active_nodes(NodeType::Agent).await;
        
        if agents.is_empty() {
            return Err(RemoteFsError::ServiceUnavailable("No agents available".to_string()));
        }
        
        // Simple round-robin selection - in a real system this could be more sophisticated
        // based on load, capability, or geographic proximity
        let index = (self.messages_routed.load(Ordering::Relaxed) as usize) % agents.len();
        Ok(agents[index].clone())
    }
    
    /// Find the target client for a response message
    async fn find_target_client_for_response(
        &self,
        message: &Message,
        state: &AppState,
    ) -> Result<String> {
        // In a real implementation, this would maintain a mapping of request IDs
        // to the originating client sessions. For now, we'll use a simplified approach.
        
        if let Some(_request_id) = message.request_id() {
            // Look up the client session that initiated this request
            // This would require maintaining a request tracking table
            
            // Simplified: just find the first available client
            let clients = state.session_manager.get_active_nodes(NodeType::Client).await;
            if clients.is_empty() {
                return Err(RemoteFsError::NotFound("No clients available".to_string()));
            }
            
            // In a real system, we'd have proper request tracking
            Ok(clients[0].clone())
        } else {
            Err(RemoteFsError::Protocol("Response message missing request ID".to_string()))
        }
    }
    
    /// Find the target for channel establishment responses
    async fn find_channel_requester(
        &self,
        _message: &Message,
        state: &AppState,
    ) -> Result<String> {
        // This would need proper tracking of channel establishment requests
        // For now, simplified implementation
        let clients = state.session_manager.get_active_nodes(NodeType::Client).await;
        if clients.is_empty() {
            return Err(RemoteFsError::NotFound("No clients available".to_string()));
        }
        
        Ok(clients[0].clone())
    }
    
    /// Find the target for error messages
    async fn find_error_target(
        &self,
        message: &Message,
        sender_session: &Session,
        state: &AppState,
    ) -> Result<String> {
        if let Message::Error { request_id, .. } = message {
            if let Some(_request_id) = request_id {
                // Find the session that originated the request
                // This would require request tracking
                
                // Simplified: route errors to the opposite type
                match sender_session.node_type {
                    NodeType::Client => {
                        let agents = state.session_manager.get_active_nodes(NodeType::Agent).await;
                        agents.first().cloned()
                            .ok_or_else(|| RemoteFsError::NotFound("No agents available".to_string()))
                    }
                    NodeType::Agent => {
                        let clients = state.session_manager.get_active_nodes(NodeType::Client).await;
                        clients.first().cloned()
                            .ok_or_else(|| RemoteFsError::NotFound("No clients available".to_string()))
                    }
                    NodeType::Relay => {
                        Err(RemoteFsError::Protocol("Relay cannot route error messages".to_string()))
                    }
                }
            } else {
                Err(RemoteFsError::Protocol("Error message missing request ID".to_string()))
            }
        } else {
            Err(RemoteFsError::Protocol("Invalid error message".to_string()))
        }
    }
    
    /// Send a message to the target node
    async fn send_to_target(
        &self,
        message: Message,
        target_node_id: &str,
        state: &AppState,
    ) -> Result<()> {
        debug!("Sending message {} to node {}", message.message_type(), target_node_id);
        
        // Get the target session
        let target_session = state.session_manager
            .get_session_by_node(target_node_id)
            .await
            .ok_or_else(|| RemoteFsError::NotFound(format!("Target node not found: {}", target_node_id)))?;
        
        // Serialize message based on the target session's preferred format
        let ws_message = match target_session.message_format {
            crate::session::MessageFormat::Json => {
                let json = serde_json::to_string(&message)
                    .map_err(|e| RemoteFsError::Protocol(format!("JSON serialization error: {}", e)))?;
                WsMessage::Text(json)
            }
            crate::session::MessageFormat::Binary => {
                let binary = bincode::serialize(&message)
                    .map_err(|e| RemoteFsError::Protocol(format!("Binary serialization error: {}", e)))?;
                WsMessage::Binary(binary)
            }
        };
        
        // Send the message
        target_session.send_message(ws_message).await?;
        
        debug!("Message {} successfully sent to {}", message.message_type(), target_node_id);
        Ok(())
    }
    
    /// Get routing statistics
    pub async fn get_stats(&self) -> RoutingStats {
        RoutingStats {
            messages_routed: self.messages_routed.load(Ordering::Relaxed),
            failed_routes: self.failed_routes.load(Ordering::Relaxed),
        }
    }
    
    /// Reset statistics (useful for testing)
    pub fn reset_stats(&self) {
        self.messages_routed.store(0, Ordering::Relaxed);
        self.failed_routes.store(0, Ordering::Relaxed);
    }
}

impl Default for MessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Request tracking entry for mapping responses back to originators
#[derive(Debug, Clone)]
pub struct RequestTrackingEntry {
    pub request_id: uuid::Uuid,
    pub originator_node_id: String,
    pub target_node_id: String,
    pub created_at: u64,
    pub message_type: String,
}

/// Enhanced message router with proper request tracking
/// This would be used in a production system
pub struct EnhancedMessageRouter {
    basic_router: MessageRouter,
    request_tracking: Arc<tokio::sync::RwLock<std::collections::HashMap<uuid::Uuid, RequestTrackingEntry>>>,
}

impl EnhancedMessageRouter {
    /// Create a new enhanced message router
    pub fn new() -> Self {
        Self {
            basic_router: MessageRouter::new(),
            request_tracking: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
    
    /// Track a request for proper response routing
    pub async fn track_request(
        &self,
        request_id: uuid::Uuid,
        originator_node_id: String,
        target_node_id: String,
        message_type: String,
    ) {
        let entry = RequestTrackingEntry {
            request_id,
            originator_node_id,
            target_node_id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            message_type,
        };
        
        let mut tracking = self.request_tracking.write().await;
        tracking.insert(request_id, entry);
    }
    
    /// Get the originator of a request
    pub async fn get_request_originator(&self, request_id: uuid::Uuid) -> Option<String> {
        let tracking = self.request_tracking.read().await;
        tracking.get(&request_id).map(|entry| entry.originator_node_id.clone())
    }
    
    /// Clean up old request tracking entries
    pub async fn cleanup_old_requests(&self, max_age_seconds: u64) -> usize {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut tracking = self.request_tracking.write().await;
        let initial_count = tracking.len();
        
        tracking.retain(|_, entry| {
            now.saturating_sub(entry.created_at) <= max_age_seconds
        });
        
        initial_count - tracking.len()
    }
    
    /// Get request tracking statistics
    pub async fn get_tracking_stats(&self) -> (usize, usize) {
        let tracking = self.request_tracking.read().await;
        let total_tracked = tracking.len();
        
        // Count by message type for debugging
        let mut type_counts = std::collections::HashMap::new();
        for entry in tracking.values() {
            *type_counts.entry(entry.message_type.clone()).or_insert(0) += 1;
        }
        
        (total_tracked, type_counts.len())
    }
}

impl Default for EnhancedMessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use remotefs_common::config_utils;
    use crate::session::Session;
    use tokio::sync::mpsc;
    
    #[tokio::test]
    async fn test_routing_stats() {
        let router = MessageRouter::new();
        
        // Initial stats should be zero
        let stats = router.get_stats().await;
        assert_eq!(stats.messages_routed, 0);
        assert_eq!(stats.failed_routes, 0);
        
        // Simulate some routing
        router.messages_routed.fetch_add(5, Ordering::Relaxed);
        router.failed_routes.fetch_add(2, Ordering::Relaxed);
        
        let stats = router.get_stats().await;
        assert_eq!(stats.messages_routed, 5);
        assert_eq!(stats.failed_routes, 2);
        
        // Test reset
        router.reset_stats();
        let stats = router.get_stats().await;
        assert_eq!(stats.messages_routed, 0);
        assert_eq!(stats.failed_routes, 0);
    }
    
    #[tokio::test]
    async fn test_enhanced_router_tracking() {
        let router = EnhancedMessageRouter::new();
        let request_id = uuid::Uuid::new_v4();
        
        // Track a request
        router.track_request(
            request_id,
            "client-001".to_string(),
            "agent-001".to_string(),
            "ReadFile".to_string(),
        ).await;
        
        // Verify tracking
        let originator = router.get_request_originator(request_id).await;
        assert_eq!(originator, Some("client-001".to_string()));
        
        // Test stats
        let (total, types) = router.get_tracking_stats().await;
        assert_eq!(total, 1);
        assert!(types >= 1);
        
        // Test cleanup (nothing should be removed yet)
        let removed = router.cleanup_old_requests(3600).await;
        assert_eq!(removed, 0);
        
        // Test cleanup with short TTL (should remove the entry)
        let removed = router.cleanup_old_requests(0).await;
        assert_eq!(removed, 1);
    }
}
