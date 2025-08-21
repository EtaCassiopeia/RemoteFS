use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use crate::error::{ClientError, ClientResult};

/// Client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// List of RemoteFS agents to connect to
    pub agents: Vec<AgentConfig>,
    
    /// Client behavior configuration
    pub client: ClientBehaviorConfig,
    
    /// Connection settings
    pub connection: ConnectionConfig,
    
    /// Authentication settings
    pub auth: Option<AuthConfig>,
    
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Configuration for a single agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent identifier
    pub id: String,
    
    /// WebSocket URL to connect to
    pub url: String,
    
    /// Optional agent-specific authentication
    pub auth: Option<AuthConfig>,
    
    /// Weight for load balancing (default: 1)
    #[serde(default = "default_weight")]
    pub weight: u32,
    
    /// Whether this agent is enabled (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

/// Client behavior configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientBehaviorConfig {
    /// Default timeout for operations (in milliseconds)
    #[serde(default = "default_operation_timeout")]
    pub operation_timeout_ms: u64,
    
    /// Maximum number of retry attempts
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    
    /// Retry backoff strategy
    #[serde(default)]
    pub retry_strategy: RetryStrategy,
    
    /// Load balancing strategy
    #[serde(default)]
    pub load_balancing: LoadBalancingStrategy,
    
    /// Enable automatic failover
    #[serde(default = "default_enabled")]
    pub enable_failover: bool,
    
    /// Buffer size for read operations
    #[serde(default = "default_read_buffer_size")]
    pub read_buffer_size: usize,
    
    /// Buffer size for write operations
    #[serde(default = "default_write_buffer_size")]
    pub write_buffer_size: usize,
}

/// Connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Connection timeout (in milliseconds)
    #[serde(default = "default_connection_timeout")]
    pub connect_timeout_ms: u64,
    
    /// Heartbeat interval (in milliseconds)
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_ms: u64,
    
    /// Maximum message size
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,
    
    /// Enable compression
    #[serde(default)]
    pub enable_compression: bool,
    
    /// Reconnection settings
    pub reconnection: ReconnectionConfig,
}

/// Reconnection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectionConfig {
    /// Enable automatic reconnection
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    
    /// Maximum reconnection attempts (0 = unlimited)
    #[serde(default = "default_max_reconnect_attempts")]
    pub max_attempts: u32,
    
    /// Base delay between reconnection attempts (in milliseconds)
    #[serde(default = "default_reconnect_delay")]
    pub base_delay_ms: u64,
    
    /// Maximum delay between reconnection attempts (in milliseconds)
    #[serde(default = "default_max_reconnect_delay")]
    pub max_delay_ms: u64,
    
    /// Delay multiplier for exponential backoff
    #[serde(default = "default_backoff_multiplier")]
    pub backoff_multiplier: f64,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Authentication method
    pub method: AuthMethod,
    
    /// Credentials
    pub credentials: AuthCredentials,
}

/// Authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "token")]
    Token,
    #[serde(rename = "certificate")]
    Certificate,
    #[serde(rename = "username_password")]
    UsernamePassword,
}

/// Authentication credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthCredentials {
    None,
    Token { token: String },
    Certificate { cert_path: PathBuf, key_path: PathBuf },
    UsernamePassword { username: String, password: String },
}

/// Retry strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetryStrategy {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "linear")]
    Linear { delay_ms: u64 },
    #[serde(rename = "exponential")]
    Exponential { base_delay_ms: u64, max_delay_ms: u64 },
}

/// Load balancing strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadBalancingStrategy {
    #[serde(rename = "round_robin")]
    RoundRobin,
    #[serde(rename = "weighted_round_robin")]
    WeightedRoundRobin,
    #[serde(rename = "least_connections")]
    LeastConnections,
    #[serde(rename = "random")]
    Random,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,
    
    /// Log format
    #[serde(default = "default_log_format")]
    pub format: String,
    
    /// Optional log file path
    pub file: Option<PathBuf>,
    
    /// Enable connection logging
    #[serde(default)]
    pub enable_connection_logs: bool,
    
    /// Enable performance logging
    #[serde(default)]
    pub enable_performance_logs: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            agents: vec![],
            client: ClientBehaviorConfig::default(),
            connection: ConnectionConfig::default(),
            auth: None,
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for ClientBehaviorConfig {
    fn default() -> Self {
        Self {
            operation_timeout_ms: default_operation_timeout(),
            max_retries: default_max_retries(),
            retry_strategy: RetryStrategy::default(),
            load_balancing: LoadBalancingStrategy::default(),
            enable_failover: default_enabled(),
            read_buffer_size: default_read_buffer_size(),
            write_buffer_size: default_write_buffer_size(),
        }
    }
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            connect_timeout_ms: default_connection_timeout(),
            heartbeat_interval_ms: default_heartbeat_interval(),
            max_message_size: default_max_message_size(),
            enable_compression: false,
            reconnection: ReconnectionConfig::default(),
        }
    }
}

impl Default for ReconnectionConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            max_attempts: default_max_reconnect_attempts(),
            base_delay_ms: default_reconnect_delay(),
            max_delay_ms: default_max_reconnect_delay(),
            backoff_multiplier: default_backoff_multiplier(),
        }
    }
}

impl Default for RetryStrategy {
    fn default() -> Self {
        RetryStrategy::Exponential {
            base_delay_ms: 1000,
            max_delay_ms: 30000,
        }
    }
}

impl Default for LoadBalancingStrategy {
    fn default() -> Self {
        LoadBalancingStrategy::RoundRobin
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            file: None,
            enable_connection_logs: false,
            enable_performance_logs: false,
        }
    }
}

impl ClientConfig {
    /// Load configuration from file
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> ClientResult<Self> {
        Self::load_from_file(&path.as_ref().to_path_buf())
    }
    
    /// Load configuration from file
    pub fn load_from_file(path: &PathBuf) -> ClientResult<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ClientError::Configuration(format!("Failed to read config file: {}", e)))?;
        
        let config: ClientConfig = match path.extension().and_then(|s| s.to_str()) {
            Some("json") => serde_json::from_str(&content)
                .map_err(|e| ClientError::Configuration(format!("Invalid JSON config: {}", e)))?,
            Some("toml") => toml::from_str(&content)
                .map_err(|e| ClientError::Configuration(format!("Invalid TOML config: {}", e)))?,
            Some("yaml") | Some("yml") => {
                return Err(ClientError::Configuration(
                    "YAML format is not supported. Please use TOML or JSON.".to_string()
                ));
            }
            _ => return Err(ClientError::Configuration(
                "Unsupported config file format. Use .json, .toml, or .yaml".to_string()
            )),
        };
        
        config.validate()?;
        Ok(config)
    }
    
    /// Save configuration to file
    pub fn save_to_file(&self, path: &PathBuf) -> ClientResult<()> {
        let content = match path.extension().and_then(|s| s.to_str()) {
            Some("json") => serde_json::to_string_pretty(self)
                .map_err(|e| ClientError::Configuration(format!("Failed to serialize JSON: {}", e)))?,
            Some("toml") => toml::to_string_pretty(self)
                .map_err(|e| ClientError::Configuration(format!("Failed to serialize TOML: {}", e)))?,
            Some("yaml") | Some("yml") => {
                return Err(ClientError::Configuration(
                    "YAML format is not supported. Please use TOML or JSON.".to_string()
                ));
            }
            _ => return Err(ClientError::Configuration(
                "Unsupported config file format. Use .json, .toml, or .yaml".to_string()
            )),
        };
        
        std::fs::write(path, content)
            .map_err(|e| ClientError::Configuration(format!("Failed to write config file: {}", e)))?;
        
        Ok(())
    }
    
    /// Validate the configuration
    pub fn validate(&self) -> ClientResult<()> {
        if self.agents.is_empty() {
            return Err(ClientError::Configuration(
                "At least one agent must be configured".to_string()
            ));
        }
        
        for agent in &self.agents {
            agent.validate()?;
        }
        
        Ok(())
    }
    
    /// Get default configuration file path
    pub fn default_config_path() -> ClientResult<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| ClientError::Configuration("Could not determine config directory".to_string()))?;
        
        Ok(config_dir.join("remotefs").join("client.toml"))
    }
    
    /// Get enabled agents
    pub fn enabled_agents(&self) -> Vec<&AgentConfig> {
        self.agents.iter().filter(|a| a.enabled).collect()
    }
    
    /// Get operation timeout as Duration
    pub fn operation_timeout(&self) -> Duration {
        Duration::from_millis(self.client.operation_timeout_ms)
    }
    
    /// Get connection timeout as Duration
    pub fn connection_timeout(&self) -> Duration {
        Duration::from_millis(self.connection.connect_timeout_ms)
    }
    
    /// Get heartbeat interval as Duration
    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.connection.heartbeat_interval_ms)
    }
}

impl ConnectionConfig {
    /// Get operation timeout as Duration
    pub fn operation_timeout(&self) -> Duration {
        Duration::from_millis(30000) // Default operation timeout
    }
    
    /// Get connection timeout as Duration
    pub fn connection_timeout(&self) -> Duration {
        Duration::from_millis(self.connect_timeout_ms)
    }
}

impl AgentConfig {
    /// Validate agent configuration
    pub fn validate(&self) -> ClientResult<()> {
        if self.id.is_empty() {
            return Err(ClientError::Configuration(
                "Agent ID cannot be empty".to_string()
            ));
        }
        
        if self.url.is_empty() {
            return Err(ClientError::Configuration(
                "Agent URL cannot be empty".to_string()
            ));
        }
        
        // Validate URL format
        url::Url::parse(&self.url)
            .map_err(|e| ClientError::Configuration(format!("Invalid agent URL '{}': {}", self.url, e)))?;
        
        if self.weight == 0 {
            return Err(ClientError::Configuration(
                "Agent weight must be greater than 0".to_string()
            ));
        }
        
        Ok(())
    }
}

// Default value functions
fn default_weight() -> u32 { 1 }
fn default_enabled() -> bool { true }
fn default_operation_timeout() -> u64 { 30000 }
fn default_max_retries() -> u32 { 3 }
fn default_read_buffer_size() -> usize { 8192 }
fn default_write_buffer_size() -> usize { 8192 }
fn default_connection_timeout() -> u64 { 10000 }
fn default_heartbeat_interval() -> u64 { 30000 }
fn default_max_message_size() -> usize { 64 * 1024 * 1024 } // 64MB
fn default_max_reconnect_attempts() -> u32 { 5 }
fn default_reconnect_delay() -> u64 { 1000 }
fn default_max_reconnect_delay() -> u64 { 30000 }
fn default_backoff_multiplier() -> f64 { 2.0 }
fn default_log_level() -> String { "info".to_string() }
fn default_log_format() -> String { "human".to_string() }
