//! RemoteFS Common Library
//! 
//! This library contains shared functionality used by all RemoteFS components:
//! - Protocol definitions for communication between client, agent, and relay
//! - Encryption and cryptography utilities 
//! - Configuration structures and handling
//! - Error types and conversions
//! - Utility functions

pub mod protocol;
pub mod crypto;
pub mod error;
pub mod config;
pub mod utils;

// Re-export commonly used types
pub use protocol::{
    Message, NodeType, ErrorCode, RequestId, NodeId, SessionToken, FsPath,
    FileMetadata, DirEntry, RelayInfo, generate_request_id,
};

pub use crypto::{
    EncryptionManager, SessionManager, EncryptedData, KeyExchange,
    generate_key, generate_keypair, derive_key_from_password,
    KeyContext, KEY_SIZE, X25519_KEY_SIZE, NONCE_SIZE, MAX_CHUNK_SIZE,
};

pub use error::{RemoteFsError, Result};

pub use config::{
    ClientConfig, AgentConfig, RelayConfig, MountPoint, MountOptions,
    CacheConfig, AccessConfig, SecurityConfig, NetworkConfig, 
    MessageLimits, SessionConfig, StorageConfig, PerformanceConfig,
    LoggingConfig, load_config, save_config,
};

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Default configuration paths
pub mod defaults {
    use std::path::PathBuf;
    
    /// Get default configuration directory
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("remotefs")
    }
    
    /// Get default cache directory  
    pub fn cache_dir() -> PathBuf {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("remotefs")
    }
    
    /// Get default data directory
    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("remotefs")
    }
    
    /// Get default client config path
    pub fn client_config_path() -> PathBuf {
        config_dir().join("client.toml")
    }
    
    /// Get default agent config path
    pub fn agent_config_path() -> PathBuf {
        config_dir().join("agent.toml")
    }
    
    /// Get default relay config path
    pub fn relay_config_path() -> PathBuf {
        config_dir().join("relay.toml")
    }
    
    /// Get default client key file path
    pub fn client_key_path() -> PathBuf {
        config_dir().join("client.key")
    }
    
    /// Get default client cert file path
    pub fn client_cert_path() -> PathBuf {
        config_dir().join("client.crt")
    }
    
    /// Get default agent key file path
    pub fn agent_key_path() -> PathBuf {
        config_dir().join("agent.key")
    }
    
    /// Get default agent cert file path
    pub fn agent_cert_path() -> PathBuf {
        config_dir().join("agent.crt")
    }
    
    /// Get default relay key file path
    pub fn relay_key_path() -> PathBuf {
        config_dir().join("relay.key")
    }
    
    /// Get default relay cert file path
    pub fn relay_cert_path() -> PathBuf {
        config_dir().join("relay.crt")
    }
}

/// Utility functions for working with configurations
pub mod config_utils {
    use crate::{config::*, defaults, Result};
    use std::path::PathBuf;
    
    /// Create default client configuration
    pub fn create_default_client_config() -> ClientConfig {
        ClientConfig {
            client_id: format!("client-{}", uuid::Uuid::new_v4()),
            relay_url: "wss://localhost:8080/ws".to_string(),
            mount_points: vec![],
            cache: CacheConfig {
                directory: defaults::cache_dir().join("client"),
                max_size_gb: 5.0,
                ttl_seconds: 3600,
                compress: true,
                encrypt: true,
            },
            security: SecurityConfig {
                key_file: defaults::client_key_path(),
                cert_file: defaults::client_cert_path(),
                enable_tls: true,
                verify_certs: true,
                session_timeout: 3600,
                enable_auth: true,
                allowed_clients: vec![],
            },
            network: NetworkConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
    
    /// Create default agent configuration
    pub fn create_default_agent_config() -> AgentConfig {
        AgentConfig {
            agent_id: format!("agent-{}", uuid::Uuid::new_v4()),
            relay_url: "wss://localhost:8080/ws".to_string(),
            access: AccessConfig {
                allowed_paths: vec!["/tmp".to_string()],
                read_only_paths: vec![],
                denied_paths: vec!["/etc".to_string(), "/root".to_string()],
                max_file_size: 10 * 1024 * 1024 * 1024, // 10GB
                follow_symlinks: true,
                allowed_extensions: vec![],
                denied_extensions: vec![],
            },
            security: SecurityConfig {
                key_file: defaults::agent_key_path(),
                cert_file: defaults::agent_cert_path(),
                enable_tls: true,
                verify_certs: true,
                session_timeout: 3600,
                enable_auth: true,
                allowed_clients: vec![],
            },
            network: NetworkConfig::default(),
            logging: LoggingConfig::default(),
            performance: PerformanceConfig {
                worker_threads: num_cpus::get(),
                io_buffer_size: 64 * 1024,
                async_io: true,
                fs_cache_size: 256,
                enable_prefetch: true,
                prefetch_window: 8,
            },
        }
    }
    
    /// Create default relay configuration
    pub fn create_default_relay_config() -> RelayConfig {
        RelayConfig {
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
            max_connections: 1000,
            message_limits: MessageLimits::default(),
            session: SessionConfig {
                timeout: 3600,
                max_sessions: 1000,
                cleanup_interval: 300,
                enable_persistence: false,
                storage_path: None,
            },
            storage: StorageConfig {
                temp_dir: defaults::data_dir().join("relay").join("temp"),
                max_size_gb: 10.0,
                temp_file_ttl: 86400,
                compress: true,
                cleanup_interval: 3600,
            },
            security: SecurityConfig {
                key_file: defaults::relay_key_path(),
                cert_file: defaults::relay_cert_path(),
                enable_tls: true,
                verify_certs: false, // Relay doesn't verify client certs by default
                session_timeout: 3600,
                enable_auth: true,
                allowed_clients: vec![],
            },
            network: NetworkConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
    
    /// Initialize configuration directory and create default configs if they don't exist
    pub fn init_configs() -> Result<()> {
        let config_dir = defaults::config_dir();
        std::fs::create_dir_all(&config_dir)?;
        
        // Create default client config if it doesn't exist
        let client_config_path = defaults::client_config_path();
        if !client_config_path.exists() {
            let config = create_default_client_config();
            save_config(&config, &client_config_path)?;
        }
        
        // Create default agent config if it doesn't exist
        let agent_config_path = defaults::agent_config_path();
        if !agent_config_path.exists() {
            let config = create_default_agent_config();
            save_config(&config, &agent_config_path)?;
        }
        
        // Create default relay config if it doesn't exist
        let relay_config_path = defaults::relay_config_path();
        if !relay_config_path.exists() {
            let config = create_default_relay_config();
            save_config(&config, &relay_config_path)?;
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_version_info() {
        assert!(!VERSION.is_empty());
        assert!(!NAME.is_empty());
    }
    
    #[test]
    fn test_default_paths() {
        let config_dir = defaults::config_dir();
        let client_config = defaults::client_config_path();
        let agent_config = defaults::agent_config_path();
        
        assert!(client_config.starts_with(&config_dir));
        assert!(agent_config.starts_with(&config_dir));
        assert!(client_config.file_name().unwrap() == "client.toml");
        assert!(agent_config.file_name().unwrap() == "agent.toml");
    }
    
    #[test]
    fn test_default_configs() {
        let client_config = config_utils::create_default_client_config();
        let agent_config = config_utils::create_default_agent_config();
        let relay_config = config_utils::create_default_relay_config();
        
        assert!(!client_config.client_id.is_empty());
        assert!(!agent_config.agent_id.is_empty());
        assert!(relay_config.port == 8080);
        
        assert!(client_config.security.enable_tls);
        assert!(agent_config.security.enable_tls);
        assert!(relay_config.security.enable_tls);
    }
}
