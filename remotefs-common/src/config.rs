use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Unique client identifier
    pub client_id: String,
    
    /// Relay server URL
    pub relay_url: String,
    
    /// Mount points configuration
    pub mount_points: Vec<MountPoint>,
    
    /// Cache configuration
    pub cache: CacheConfig,
    
    /// Security configuration
    pub security: SecurityConfig,
    
    /// Network configuration
    pub network: NetworkConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Unique agent identifier
    pub agent_id: String,
    
    /// Relay server URL
    pub relay_url: String,
    
    /// Access control configuration
    pub access: AccessConfig,
    
    /// Security configuration
    pub security: SecurityConfig,
    
    /// Network configuration
    pub network: NetworkConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
    
    /// Performance tuning
    pub performance: PerformanceConfig,
}

/// Relay server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// Server bind address
    pub bind_address: String,
    
    /// Server port
    pub port: u16,
    
    /// Maximum number of concurrent connections
    pub max_connections: usize,
    
    /// Message size limits
    pub message_limits: MessageLimits,
    
    /// Session configuration
    pub session: SessionConfig,
    
    /// Storage configuration for temporary data
    pub storage: StorageConfig,
    
    /// Security configuration
    pub security: SecurityConfig,
    
    /// Network configuration
    pub network: NetworkConfig,
    
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Mount point configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountPoint {
    /// Remote path on the agent
    pub remote_path: String,
    
    /// Local mount path
    pub local_path: PathBuf,
    
    /// Mount options
    pub options: MountOptions,
    
    /// Target agent ID
    pub agent_id: String,
}

/// Mount options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountOptions {
    /// Mount as read-only
    #[serde(default)]
    pub read_only: bool,
    
    /// Enable write caching
    #[serde(default = "default_true")]
    pub write_cache: bool,
    
    /// Enable read caching
    #[serde(default = "default_true")]
    pub read_cache: bool,
    
    /// Cache TTL in seconds
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl: u64,
    
    /// Maximum file size to cache (in bytes)
    #[serde(default = "default_max_cached_file_size")]
    pub max_cached_file_size: u64,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Cache directory
    pub directory: PathBuf,
    
    /// Maximum cache size in GB
    pub max_size_gb: f64,
    
    /// Cache TTL in seconds
    #[serde(default = "default_cache_ttl")]
    pub ttl_seconds: u64,
    
    /// Enable compression for cached data
    #[serde(default = "default_true")]
    pub compress: bool,
    
    /// Enable encryption for cached data
    #[serde(default = "default_true")]
    pub encrypt: bool,
}

/// Access control configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessConfig {
    /// Allowed paths that can be accessed
    pub allowed_paths: Vec<String>,
    
    /// Paths that are read-only
    #[serde(default)]
    pub read_only_paths: Vec<String>,
    
    /// Denied paths (higher priority than allowed)
    #[serde(default)]
    pub denied_paths: Vec<String>,
    
    /// Maximum file size that can be accessed
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    
    /// Enable symlink following
    #[serde(default = "default_true")]
    pub follow_symlinks: bool,
    
    /// Allowed file extensions (empty = allow all)
    #[serde(default)]
    pub allowed_extensions: Vec<String>,
    
    /// Denied file extensions
    #[serde(default)]
    pub denied_extensions: Vec<String>,
}

/// Security configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Path to private key file
    pub key_file: PathBuf,
    
    /// Path to certificate file
    pub cert_file: PathBuf,
    
    /// Enable TLS for connections
    #[serde(default = "default_true")]
    pub enable_tls: bool,
    
    /// Verify TLS certificates
    #[serde(default = "default_true")]
    pub verify_certs: bool,
    
    /// Session timeout in seconds
    #[serde(default = "default_session_timeout")]
    pub session_timeout: u64,
    
    /// Enable authentication
    #[serde(default = "default_true")]
    pub enable_auth: bool,
    
    /// Allowed client certificates (for mutual TLS)
    #[serde(default)]
    pub allowed_clients: Vec<String>,
}

/// Network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    
    /// Read timeout in seconds
    #[serde(default = "default_io_timeout")]
    pub read_timeout: u64,
    
    /// Write timeout in seconds
    #[serde(default = "default_io_timeout")]
    pub write_timeout: u64,
    
    /// Heartbeat interval in seconds
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval: u64,
    
    /// Maximum reconnection attempts
    #[serde(default = "default_max_reconnect_attempts")]
    pub max_reconnect_attempts: u32,
    
    /// Reconnection backoff base in seconds
    #[serde(default = "default_reconnect_backoff")]
    pub reconnect_backoff_base: u64,
    
    /// Maximum concurrent connections per client/agent
    #[serde(default = "default_max_concurrent_connections")]
    pub max_concurrent_connections: usize,
    
    /// Enable TCP keep-alive
    #[serde(default = "default_true")]
    pub tcp_keepalive: bool,
    
    /// TCP keep-alive interval in seconds
    #[serde(default = "default_keepalive_interval")]
    pub keepalive_interval: u64,
}

/// Message size limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageLimits {
    /// Maximum message size in bytes
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,
    
    /// Maximum file chunk size in bytes
    #[serde(default = "default_max_chunk_size")]
    pub max_chunk_size: usize,
    
    /// Maximum number of directory entries per response
    #[serde(default = "default_max_dir_entries")]
    pub max_dir_entries: usize,
}

/// Session configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Session timeout in seconds
    #[serde(default = "default_session_timeout")]
    pub timeout: u64,
    
    /// Maximum concurrent sessions
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,
    
    /// Session cleanup interval in seconds
    #[serde(default = "default_session_cleanup_interval")]
    pub cleanup_interval: u64,
    
    /// Enable session persistence
    #[serde(default)]
    pub enable_persistence: bool,
    
    /// Session storage path (for persistence)
    pub storage_path: Option<PathBuf>,
}

/// Storage configuration for relay server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Temporary storage directory
    pub temp_dir: PathBuf,
    
    /// Maximum temporary storage size in GB
    #[serde(default = "default_temp_storage_size")]
    pub max_size_gb: f64,
    
    /// Temporary file TTL in seconds
    #[serde(default = "default_temp_file_ttl")]
    pub temp_file_ttl: u64,
    
    /// Enable compression for temporary files
    #[serde(default = "default_true")]
    pub compress: bool,
    
    /// Cleanup interval in seconds
    #[serde(default = "default_cleanup_interval")]
    pub cleanup_interval: u64,
}

/// Performance tuning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Number of worker threads
    #[serde(default = "default_worker_threads")]
    pub worker_threads: usize,
    
    /// I/O buffer size in bytes
    #[serde(default = "default_io_buffer_size")]
    pub io_buffer_size: usize,
    
    /// Enable async I/O
    #[serde(default = "default_true")]
    pub async_io: bool,
    
    /// File system cache size in MB
    #[serde(default = "default_fs_cache_size")]
    pub fs_cache_size: usize,
    
    /// Enable prefetching
    #[serde(default = "default_true")]
    pub enable_prefetch: bool,
    
    /// Prefetch window size
    #[serde(default = "default_prefetch_window")]
    pub prefetch_window: usize,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub level: String,
    
    /// Log format (json, plain)
    #[serde(default = "default_log_format")]
    pub format: String,
    
    /// Log file path (None = stdout)
    pub file: Option<PathBuf>,
    
    /// Maximum log file size in MB
    #[serde(default = "default_log_file_size")]
    pub max_file_size: usize,
    
    /// Maximum number of log files to keep
    #[serde(default = "default_log_file_count")]
    pub max_files: usize,
    
    /// Enable access logging
    #[serde(default)]
    pub enable_access_log: bool,
    
    /// Access log file path
    pub access_log_file: Option<PathBuf>,
}

// Default value functions
fn default_true() -> bool { true }
fn default_cache_ttl() -> u64 { 3600 } // 1 hour
fn default_max_cached_file_size() -> u64 { 100 * 1024 * 1024 } // 100MB
fn default_max_file_size() -> u64 { 10 * 1024 * 1024 * 1024 } // 10GB
fn default_session_timeout() -> u64 { 3600 } // 1 hour
fn default_connection_timeout() -> u64 { 30 } // 30 seconds
fn default_io_timeout() -> u64 { 60 } // 1 minute
fn default_heartbeat_interval() -> u64 { 30 } // 30 seconds
fn default_max_reconnect_attempts() -> u32 { 5 }
fn default_reconnect_backoff() -> u64 { 1 } // 1 second
fn default_max_concurrent_connections() -> usize { 10 }
fn default_keepalive_interval() -> u64 { 60 } // 1 minute
fn default_max_message_size() -> usize { 64 * 1024 * 1024 } // 64MB
fn default_max_chunk_size() -> usize { 1024 * 1024 } // 1MB
fn default_max_dir_entries() -> usize { 1000 }
fn default_max_sessions() -> usize { 1000 }
fn default_session_cleanup_interval() -> u64 { 300 } // 5 minutes
fn default_temp_storage_size() -> f64 { 10.0 } // 10GB
fn default_temp_file_ttl() -> u64 { 86400 } // 24 hours
fn default_cleanup_interval() -> u64 { 3600 } // 1 hour
fn default_worker_threads() -> usize { num_cpus::get() }
fn default_io_buffer_size() -> usize { 64 * 1024 } // 64KB
fn default_fs_cache_size() -> usize { 256 } // 256MB
fn default_prefetch_window() -> usize { 8 }
fn default_log_level() -> String { "info".to_string() }
fn default_log_format() -> String { "plain".to_string() }
fn default_log_file_size() -> usize { 100 } // 100MB
fn default_log_file_count() -> usize { 5 }

impl Default for MountOptions {
    fn default() -> Self {
        Self {
            read_only: false,
            write_cache: true,
            read_cache: true,
            cache_ttl: default_cache_ttl(),
            max_cached_file_size: default_max_cached_file_size(),
        }
    }
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            connection_timeout: default_connection_timeout(),
            read_timeout: default_io_timeout(),
            write_timeout: default_io_timeout(),
            heartbeat_interval: default_heartbeat_interval(),
            max_reconnect_attempts: default_max_reconnect_attempts(),
            reconnect_backoff_base: default_reconnect_backoff(),
            max_concurrent_connections: default_max_concurrent_connections(),
            tcp_keepalive: true,
            keepalive_interval: default_keepalive_interval(),
        }
    }
}

impl Default for MessageLimits {
    fn default() -> Self {
        Self {
            max_message_size: default_max_message_size(),
            max_chunk_size: default_max_chunk_size(),
            max_dir_entries: default_max_dir_entries(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            file: None,
            max_file_size: default_log_file_size(),
            max_files: default_log_file_count(),
            enable_access_log: false,
            access_log_file: None,
        }
    }
}

/// Load configuration from file
pub fn load_config<T>(config_path: &PathBuf) -> crate::error::Result<T> 
where
    T: serde::de::DeserializeOwned,
{
    let content = std::fs::read_to_string(config_path)
        .map_err(|e| crate::error::RemoteFsError::Configuration(
            format!("Failed to read config file {}: {}", config_path.display(), e)
        ))?;
        
    toml::from_str(&content)
        .map_err(|e| crate::error::RemoteFsError::Configuration(
            format!("Failed to parse config file {}: {}", config_path.display(), e)
        ))
}

/// Save configuration to file
pub fn save_config<T>(config: &T, config_path: &PathBuf) -> crate::error::Result<()>
where
    T: serde::Serialize,
{
    let content = toml::to_string_pretty(config)
        .map_err(|e| crate::error::RemoteFsError::Configuration(
            format!("Failed to serialize config: {}", e)
        ))?;
        
    // Create parent directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| crate::error::RemoteFsError::Configuration(
                format!("Failed to create config directory {}: {}", parent.display(), e)
            ))?;
    }
    
    std::fs::write(config_path, content)
        .map_err(|e| crate::error::RemoteFsError::Configuration(
            format!("Failed to write config file {}: {}", config_path.display(), e)
        ))?;
        
    Ok(())
}

/// Load client configuration from file
pub fn load_client_config<P: AsRef<std::path::Path>>(path: P) -> crate::error::Result<ClientConfig> {
    let content = std::fs::read_to_string(path.as_ref())
        .map_err(|e| crate::error::RemoteFsError::Configuration(
            format!("Failed to read client config: {}", e)
        ))?;
        
    toml::from_str(&content)
        .map_err(|e| crate::error::RemoteFsError::Configuration(
            format!("Failed to parse client config: {}", e)
        ))
}

/// Load agent configuration from file
pub fn load_agent_config<P: AsRef<std::path::Path>>(path: P) -> crate::error::Result<AgentConfig> {
    let content = std::fs::read_to_string(path.as_ref())
        .map_err(|e| crate::error::RemoteFsError::Configuration(
            format!("Failed to read agent config: {}", e)
        ))?;
        
    toml::from_str(&content)
        .map_err(|e| crate::error::RemoteFsError::Configuration(
            format!("Failed to parse agent config: {}", e)
        ))
}

/// Load relay configuration from file
pub fn load_relay_config<P: AsRef<std::path::Path>>(path: P) -> crate::error::Result<RelayConfig> {
    let content = std::fs::read_to_string(path.as_ref())
        .map_err(|e| crate::error::RemoteFsError::Configuration(
            format!("Failed to read relay config: {}", e)
        ))?;
        
    toml::from_str(&content)
        .map_err(|e| crate::error::RemoteFsError::Configuration(
            format!("Failed to parse relay config: {}", e)
        ))
}
