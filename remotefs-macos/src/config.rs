use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the macOS RemoteFS NFS server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacOSConfig {
    /// NFS server bind address
    pub host: String,
    
    /// NFS server port
    pub port: u16,
    
    /// RemoteFS agent endpoints to connect to
    pub agents: Vec<String>,
    
    /// Connection timeout in seconds
    pub connection_timeout: u64,
    
    /// Request timeout in seconds  
    pub request_timeout: u64,
    
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    
    /// Enable debug logging
    pub debug: bool,
    
    /// Path to store temporary data
    pub cache_dir: Option<PathBuf>,
    
    /// Authentication settings
    pub auth: AuthConfig,
    
    /// Performance settings
    pub performance: PerformanceConfig,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable authentication
    pub enabled: bool,
    
    /// Authentication token (if using token-based auth)
    pub token: Option<String>,
    
    /// Path to certificate file for TLS
    pub cert_file: Option<PathBuf>,
    
    /// Path to private key file for TLS
    pub key_file: Option<PathBuf>,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Enable caching
    pub cache_enabled: bool,
    
    /// Cache size in MB
    pub cache_size_mb: u64,
    
    /// Read buffer size in bytes
    pub read_buffer_size: usize,
    
    /// Write buffer size in bytes
    pub write_buffer_size: usize,
    
    /// Connection pool size
    pub connection_pool_size: usize,
    
    /// Enable compression
    pub compression_enabled: bool,
}

impl Default for MacOSConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 2049,
            agents: vec!["ws://127.0.0.1:8080".to_string()],
            connection_timeout: 30,
            request_timeout: 60,
            max_connections: 100,
            debug: false,
            cache_dir: None,
            auth: AuthConfig::default(),
            performance: PerformanceConfig::default(),
        }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            token: None,
            cert_file: None,
            key_file: None,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            cache_enabled: true,
            cache_size_mb: 256,
            read_buffer_size: 64 * 1024,  // 64KB
            write_buffer_size: 64 * 1024, // 64KB
            connection_pool_size: 10,
            compression_enabled: true,
        }
    }
}

impl MacOSConfig {
    /// Load configuration from a TOML file
    pub fn from_file(path: &PathBuf) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to read config file {}: {}", path.display(), e)
            ))?;
            
        Self::from_toml(&content)
    }
    
    /// Load configuration from TOML string
    pub fn from_toml(content: &str) -> crate::Result<Self> {
        toml::from_str(content)
            .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to parse config: {}", e)
            ))
    }
    
    /// Save configuration to a TOML file
    pub fn save_to_file(&self, path: &PathBuf) -> crate::Result<()> {
        let content = self.to_toml()?;
        std::fs::write(path, content)
            .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to write config file {}: {}", path.display(), e)
            ))
    }
    
    /// Convert configuration to TOML string
    pub fn to_toml(&self) -> crate::Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to serialize config: {}", e)
            ))
    }
    
    /// Get default configuration file path
    pub fn default_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("remotefs")
            .join("macos.toml")
    }
    
    /// Load configuration with fallback to defaults
    pub fn load_or_default() -> Self {
        let config_path = Self::default_config_path();
        
        if config_path.exists() {
            match Self::from_file(&config_path) {
                Ok(config) => {
                    println!("Loaded configuration from {}", config_path.display());
                    config
                }
                Err(e) => {
                    eprintln!("Failed to load config from {}: {}", config_path.display(), e);
                    eprintln!("Using default configuration");
                    Self::default()
                }
            }
        } else {
            println!("Config file not found, using defaults");
            Self::default()
        }
    }
    
    /// Create example configuration file
    pub fn create_example_config() -> crate::Result<()> {
        let config_path = Self::default_config_path();
        
        // Create directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                    format!("Failed to create config directory: {}", e)
                ))?;
        }
        
        let example_config = Self::example_config();
        example_config.save_to_file(&config_path)?;
        
        println!("Created example configuration at {}", config_path.display());
        Ok(())
    }
    
    /// Get example configuration with comments
    fn example_config() -> Self {
        Self {
            host: "0.0.0.0".to_string(), // Listen on all interfaces
            port: 2049,
            agents: vec![
                "ws://127.0.0.1:8080".to_string(),
                "ws://remote-agent:8080".to_string(),
            ],
            connection_timeout: 30,
            request_timeout: 120,
            max_connections: 200,
            debug: false,
            cache_dir: Some(PathBuf::from("/tmp/remotefs-cache")),
            auth: AuthConfig {
                enabled: true,
                token: Some("your-auth-token-here".to_string()),
                cert_file: Some(PathBuf::from("/path/to/cert.pem")),
                key_file: Some(PathBuf::from("/path/to/key.pem")),
            },
            performance: PerformanceConfig {
                cache_enabled: true,
                cache_size_mb: 512,
                read_buffer_size: 128 * 1024,  // 128KB
                write_buffer_size: 128 * 1024, // 128KB
                connection_pool_size: 20,
                compression_enabled: true,
            },
        }
    }
    
    /// Validate configuration
    pub fn validate(&self) -> crate::Result<()> {
        if self.agents.is_empty() {
            return Err(remotefs_common::error::RemoteFsError::Internal(
                "At least one agent must be specified".to_string()
            ));
        }
        
        if self.port == 0 {
            return Err(remotefs_common::error::RemoteFsError::Internal(
                "Port must be greater than 0".to_string()
            ));
        }
        
        if self.connection_timeout == 0 || self.request_timeout == 0 {
            return Err(remotefs_common::error::RemoteFsError::Internal(
                "Timeouts must be greater than 0".to_string()
            ));
        }
        
        // Validate agent URLs
        for agent in &self.agents {
            if !agent.starts_with("ws://") && !agent.starts_with("wss://") {
                return Err(remotefs_common::error::RemoteFsError::Internal(
                    format!("Invalid agent URL (must start with ws:// or wss://): {}", agent)
                ));
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_default_config() {
        let config = MacOSConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 2049);
        assert!(!config.agents.is_empty());
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_config_serialization() {
        let config = MacOSConfig::default();
        let toml_str = config.to_toml().unwrap();
        let parsed_config = MacOSConfig::from_toml(&toml_str).unwrap();
        
        assert_eq!(config.host, parsed_config.host);
        assert_eq!(config.port, parsed_config.port);
        assert_eq!(config.agents, parsed_config.agents);
    }
    
    #[test]
    fn test_config_file_operations() {
        let config = MacOSConfig::default();
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path().to_path_buf();
        
        // Save config
        config.save_to_file(&temp_path).unwrap();
        
        // Load config
        let loaded_config = MacOSConfig::from_file(&temp_path).unwrap();
        assert_eq!(config.host, loaded_config.host);
        assert_eq!(config.port, loaded_config.port);
    }
    
    #[test]
    fn test_config_validation() {
        // Valid config
        let config = MacOSConfig::default();
        assert!(config.validate().is_ok());
        
        // Invalid config - no agents
        let mut invalid_config = MacOSConfig::default();
        invalid_config.agents.clear();
        assert!(invalid_config.validate().is_err());
        
        // Invalid config - port 0
        let mut invalid_config = MacOSConfig::default();
        invalid_config.port = 0;
        assert!(invalid_config.validate().is_err());
        
        // Invalid config - bad agent URL
        let mut invalid_config = MacOSConfig::default();
        invalid_config.agents = vec!["http://invalid".to_string()];
        assert!(invalid_config.validate().is_err());
    }
}
