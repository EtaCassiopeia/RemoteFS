use remotefs_common::{
    protocol::{NodeType, SessionToken},
    config::RelayConfig,
    error::{RemoteFsError, Result},
    crypto::{generate_key, EncryptionManager},
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};
use uuid::Uuid;

/// Authentication manager for the relay server
pub struct AuthManager {
    config: RelayConfig,
    active_tokens: Arc<RwLock<HashMap<String, AuthenticatedNode>>>,
    encryption_manager: Arc<EncryptionManager>,
}

/// Represents an authenticated node
#[derive(Debug, Clone)]
pub struct AuthenticatedNode {
    pub node_id: String,
    pub node_type: NodeType,
    pub session_token: SessionToken,
    pub public_key: Vec<u8>,
    pub capabilities: Vec<String>,
    pub authenticated_at: u64,
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new(config: &RelayConfig) -> Self {
        let master_key = generate_key();
        let encryption_manager = Arc::new(EncryptionManager::new(master_key));
        
        Self {
            config: config.clone(),
            active_tokens: Arc::new(RwLock::new(HashMap::new())),
            encryption_manager,
        }
    }
    
    /// Authenticate a node (client or agent)
    pub async fn authenticate_node(
        &self,
        node_id: &str,
        node_type: &NodeType,
        public_key: &[u8],
        capabilities: &[String],
    ) -> Result<SessionToken> {
        debug!("Authenticating node: {} ({:?})", node_id, node_type);
        
        // Validate node ID format
        self.validate_node_id(node_id)?;
        
        // Validate node type
        self.validate_node_type(node_type)?;
        
        // Validate public key
        self.validate_public_key(public_key)?;
        
        // Validate capabilities
        self.validate_capabilities(capabilities)?;
        
        // Check if authentication is enabled
        if !self.config.security.enable_auth {
            debug!("Authentication disabled, allowing node: {}", node_id);
            return Ok(self.generate_session_token(node_id));
        }
        
        // In a real system, this would involve more sophisticated authentication:
        // - Certificate validation
        // - Pre-shared keys
        // - OAuth/OIDC integration
        // - Challenge-response authentication
        
        // For now, we implement basic validation
        let authenticated = self.perform_basic_authentication(node_id, node_type, public_key).await?;
        
        if authenticated {
            // Generate session token
            let session_token = self.generate_session_token(node_id);
            
            // Store authenticated node
            let authenticated_node = AuthenticatedNode {
                node_id: node_id.to_string(),
                node_type: node_type.clone(),
                session_token: session_token.clone(),
                public_key: public_key.to_vec(),
                capabilities: capabilities.to_vec(),
                authenticated_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            };
            
            let mut tokens = self.active_tokens.write().await;
            tokens.insert(session_token.clone(), authenticated_node);
            
            debug!("Node {} authenticated successfully", node_id);
            Ok(session_token)
        } else {
            Err(RemoteFsError::Authentication(
                format!("Authentication failed for node: {}", node_id)
            ))
        }
    }
    
    /// Validate a session token
    pub async fn validate_token(&self, session_token: &str) -> Result<AuthenticatedNode> {
        let tokens = self.active_tokens.read().await;
        
        if let Some(authenticated_node) = tokens.get(session_token) {
            // Check if token is expired
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            if now.saturating_sub(authenticated_node.authenticated_at) > self.config.security.session_timeout {
                warn!("Session token expired for node: {}", authenticated_node.node_id);
                return Err(RemoteFsError::Authentication("Session token expired".to_string()));
            }
            
            Ok(authenticated_node.clone())
        } else {
            Err(RemoteFsError::Authentication("Invalid session token".to_string()))
        }
    }
    
    /// Revoke a session token
    pub async fn revoke_token(&self, session_token: &str) -> Result<()> {
        let mut tokens = self.active_tokens.write().await;
        
        if let Some(authenticated_node) = tokens.remove(session_token) {
            debug!("Revoked session token for node: {}", authenticated_node.node_id);
            Ok(())
        } else {
            Err(RemoteFsError::NotFound("Session token not found".to_string()))
        }
    }
    
    /// Clean up expired tokens
    pub async fn cleanup_expired_tokens(&self) -> usize {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let mut tokens = self.active_tokens.write().await;
        let initial_count = tokens.len();
        
        tokens.retain(|_token, node| {
            now.saturating_sub(node.authenticated_at) <= self.config.security.session_timeout
        });
        
        let removed = initial_count - tokens.len();
        if removed > 0 {
            debug!("Cleaned up {} expired authentication tokens", removed);
        }
        
        removed
    }
    
    /// Get authentication statistics
    pub async fn get_auth_stats(&self) -> AuthStats {
        let tokens = self.active_tokens.read().await;
        let total_authenticated = tokens.len();
        
        let mut clients = 0;
        let mut agents = 0;
        
        for node in tokens.values() {
            match node.node_type {
                NodeType::Client => clients += 1,
                NodeType::Agent => agents += 1,
                NodeType::Relay => {} // Relays don't authenticate with other relays
            }
        }
        
        AuthStats {
            total_authenticated,
            authenticated_clients: clients,
            authenticated_agents: agents,
        }
    }
    
    /// Validate node ID format
    fn validate_node_id(&self, node_id: &str) -> Result<()> {
        if node_id.is_empty() {
            return Err(RemoteFsError::Authentication("Node ID cannot be empty".to_string()));
        }
        
        if node_id.len() > 64 {
            return Err(RemoteFsError::Authentication("Node ID too long".to_string()));
        }
        
        // Only allow alphanumeric characters, hyphens, and underscores
        if !node_id.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(RemoteFsError::Authentication(
                "Node ID contains invalid characters".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Validate node type
    fn validate_node_type(&self, node_type: &NodeType) -> Result<()> {
        match node_type {
            NodeType::Client | NodeType::Agent => Ok(()),
            NodeType::Relay => Err(RemoteFsError::Authentication(
                "Relay nodes cannot authenticate with other relays".to_string()
            )),
        }
    }
    
    /// Validate public key format
    fn validate_public_key(&self, public_key: &[u8]) -> Result<()> {
        if public_key.is_empty() {
            return Err(RemoteFsError::Authentication("Public key cannot be empty".to_string()));
        }
        
        // For X25519, public keys should be exactly 32 bytes
        if public_key.len() != 32 {
            return Err(RemoteFsError::Authentication(
                format!("Invalid public key length: expected 32 bytes, got {}", public_key.len())
            ));
        }
        
        Ok(())
    }
    
    /// Validate capabilities
    fn validate_capabilities(&self, capabilities: &[String]) -> Result<()> {
        const MAX_CAPABILITIES: usize = 20;
        const MAX_CAPABILITY_LENGTH: usize = 64;
        
        if capabilities.len() > MAX_CAPABILITIES {
            return Err(RemoteFsError::Authentication(
                format!("Too many capabilities: max {}, got {}", MAX_CAPABILITIES, capabilities.len())
            ));
        }
        
        for capability in capabilities {
            if capability.is_empty() {
                return Err(RemoteFsError::Authentication("Empty capability not allowed".to_string()));
            }
            
            if capability.len() > MAX_CAPABILITY_LENGTH {
                return Err(RemoteFsError::Authentication(
                    format!("Capability too long: max {} characters", MAX_CAPABILITY_LENGTH)
                ));
            }
            
            // Only allow alphanumeric characters, hyphens, underscores, and dots
            if !capability.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
                return Err(RemoteFsError::Authentication(
                    format!("Invalid capability '{}': contains invalid characters", capability)
                ));
            }
        }
        
        Ok(())
    }
    
    /// Perform basic authentication (placeholder for more sophisticated auth)
    async fn perform_basic_authentication(
        &self,
        node_id: &str,
        node_type: &NodeType,
        _public_key: &[u8],
    ) -> Result<bool> {
        // In a real system, this would involve:
        // 1. Certificate validation
        // 2. Signature verification
        // 3. Checking against allowed node lists
        // 4. Rate limiting
        // 5. IP allowlisting/denylisting
        
        // For now, implement basic rules:
        
        // Check if node is in the allowed clients list (if specified)
        if !self.config.security.allowed_clients.is_empty() {
            let is_allowed = self.config.security.allowed_clients
                .iter()
                .any(|allowed| allowed == node_id);
                
            if !is_allowed {
                warn!("Node {} not in allowed clients list", node_id);
                return Ok(false);
            }
        }
        
        // Basic node type validation
        match node_type {
            NodeType::Client => {
                // Clients should have IDs starting with "client-" (optional convention)
                if node_id.starts_with("client-") || self.config.security.allowed_clients.is_empty() {
                    Ok(true)
                } else {
                    warn!("Client node ID '{}' doesn't follow naming convention", node_id);
                    Ok(false)
                }
            }
            NodeType::Agent => {
                // Agents should have IDs starting with "agent-" (optional convention)
                if node_id.starts_with("agent-") || self.config.security.allowed_clients.is_empty() {
                    Ok(true)
                } else {
                    warn!("Agent node ID '{}' doesn't follow naming convention", node_id);
                    Ok(false)
                }
            }
            NodeType::Relay => Ok(false), // Relays shouldn't authenticate with each other
        }
    }
    
    /// Generate a session token
    fn generate_session_token(&self, node_id: &str) -> SessionToken {
        format!("{}_{}", Uuid::new_v4(), node_id)
    }
}

/// Authentication statistics
#[derive(Debug, Clone)]
pub struct AuthStats {
    pub total_authenticated: usize,
    pub authenticated_clients: usize,
    pub authenticated_agents: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use remotefs_common::config_utils;
    
    #[tokio::test]
    async fn test_authentication_flow() {
        let config = config_utils::create_default_relay_config();
        let auth_manager = AuthManager::new(&config);
        
        let node_id = "client-test-001";
        let node_type = NodeType::Client;
        let public_key = vec![0u8; 32]; // Mock 32-byte public key
        let capabilities = vec!["read".to_string(), "write".to_string()];
        
        // Test successful authentication
        let token = auth_manager
            .authenticate_node(node_id, &node_type, &public_key, &capabilities)
            .await
            .expect("Authentication should succeed");
        
        assert!(!token.is_empty());
        
        // Test token validation
        let authenticated_node = auth_manager
            .validate_token(&token)
            .await
            .expect("Token validation should succeed");
        
        assert_eq!(authenticated_node.node_id, node_id);
        assert_eq!(authenticated_node.capabilities, capabilities);
        
        // Test token revocation
        auth_manager
            .revoke_token(&token)
            .await
            .expect("Token revocation should succeed");
        
        // Token should no longer be valid
        let validation_result = auth_manager.validate_token(&token).await;
        assert!(validation_result.is_err());
    }
    
    #[tokio::test]
    async fn test_validation_failures() {
        let config = config_utils::create_default_relay_config();
        let auth_manager = AuthManager::new(&config);
        
        // Test empty node ID
        let result = auth_manager
            .authenticate_node("", &NodeType::Client, &vec![0u8; 32], &vec![])
            .await;
        assert!(result.is_err());
        
        // Test invalid public key length
        let result = auth_manager
            .authenticate_node("client-test", &NodeType::Client, &vec![0u8; 16], &vec![])
            .await;
        assert!(result.is_err());
        
        // Test too many capabilities
        let many_caps: Vec<String> = (0..25).map(|i| format!("cap-{}", i)).collect();
        let result = auth_manager
            .authenticate_node("client-test", &NodeType::Client, &vec![0u8; 32], &many_caps)
            .await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_token_expiration() {
        let mut config = config_utils::create_default_relay_config();
        config.security.session_timeout = 1; // 1 second timeout
        
        let auth_manager = AuthManager::new(&config);
        
        // Authenticate a node
        let token = auth_manager
            .authenticate_node(
                "client-expire-test",
                &NodeType::Client,
                &vec![0u8; 32],
                &vec![],
            )
            .await
            .expect("Authentication should succeed");
        
        // Token should be valid initially
        let validation_result = auth_manager.validate_token(&token).await;
        assert!(validation_result.is_ok());
        
        // Wait for expiration
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        
        // Token should now be expired
        let validation_result = auth_manager.validate_token(&token).await;
        assert!(validation_result.is_err());
        
        // Cleanup should remove expired tokens
        let removed = auth_manager.cleanup_expired_tokens().await;
        assert_eq!(removed, 1);
    }
    
    #[tokio::test]
    async fn test_auth_stats() {
        let config = config_utils::create_default_relay_config();
        let auth_manager = AuthManager::new(&config);
        
        // Initial stats should be zero
        let stats = auth_manager.get_auth_stats().await;
        assert_eq!(stats.total_authenticated, 0);
        assert_eq!(stats.authenticated_clients, 0);
        assert_eq!(stats.authenticated_agents, 0);
        
        // Authenticate some nodes
        let _client_token = auth_manager
            .authenticate_node("client-001", &NodeType::Client, &vec![0u8; 32], &vec![])
            .await
            .expect("Client authentication should succeed");
            
        let _agent_token = auth_manager
            .authenticate_node("agent-001", &NodeType::Agent, &vec![0u8; 32], &vec![])
            .await
            .expect("Agent authentication should succeed");
        
        // Check updated stats
        let stats = auth_manager.get_auth_stats().await;
        assert_eq!(stats.total_authenticated, 2);
        assert_eq!(stats.authenticated_clients, 1);
        assert_eq!(stats.authenticated_agents, 1);
    }
}
