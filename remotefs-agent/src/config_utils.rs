use std::path::{Path, PathBuf};
use std::fs;
use remotefs_common::{
    config::{AgentConfig, AccessConfig, SecurityConfig, NetworkConfig, LoggingConfig, PerformanceConfig},
    error::{RemoteFsError, Result},
};
use dirs;

/// Create a default agent configuration
pub fn create_default_agent_config() -> AgentConfig {
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let config_dir = home_dir.join(".remotefs");
    
    AgentConfig {
        agent_id: format!("agent-{}", uuid::Uuid::new_v4()),
        relay_url: "ws://localhost:8080/ws".to_string(),
        access: AccessConfig {
            allowed_paths: vec![
                home_dir.join("Documents").to_string_lossy().to_string(),
                home_dir.join("Downloads").to_string_lossy().to_string(),
            ],
            read_only_paths: vec![],
            denied_paths: vec![
                "/etc".to_string(),
                "/root".to_string(),
                "/sys".to_string(),
                "/proc".to_string(),
            ],
            max_file_size: 100 * 1024 * 1024, // 100MB
            follow_symlinks: false,
            allowed_extensions: vec![],
            denied_extensions: vec![
                "exe".to_string(),
                "bat".to_string(),
                "cmd".to_string(),
                "scr".to_string(),
            ],
        },
        security: SecurityConfig {
            key_file: config_dir.join("agent.key"),
            cert_file: config_dir.join("agent.crt"),
            enable_tls: true,
            verify_certs: true,
            session_timeout: 3600,
            enable_auth: true,
            allowed_clients: vec![],
        },
        network: NetworkConfig::default(),
        logging: LoggingConfig {
            level: "info".to_string(),
            format: "json".to_string(),
            file: Some(config_dir.join("agent.log")),
            max_file_size: 10,
            max_files: 5,
            enable_access_log: true,
            access_log_file: Some(config_dir.join("access.log")),
        },
        performance: PerformanceConfig {
            worker_threads: num_cpus::get(),
            io_buffer_size: 64 * 1024,
            async_io: true,
            fs_cache_size: 128,
            enable_prefetch: true,
            prefetch_window: 8,
        },
    }
}

/// Load configuration from a file
pub fn load_config_from_file<P: AsRef<Path>>(path: P) -> Result<AgentConfig> {
    let content = fs::read_to_string(path.as_ref())
        .map_err(|e| RemoteFsError::Configuration(format!(
            "Failed to read config file {}: {}",
            path.as_ref().display(),
            e
        )))?;
    
    toml::from_str(&content)
        .map_err(|e| RemoteFsError::Configuration(format!(
            "Failed to parse config file {}: {}",
            path.as_ref().display(),
            e
        )))
}

/// Save configuration to a file
pub fn save_config_to_file<P: AsRef<Path>>(config: &AgentConfig, path: P) -> Result<()> {
    // Create parent directory if it doesn't exist
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)
            .map_err(|e| RemoteFsError::Configuration(format!(
                "Failed to create config directory {}: {}",
                parent.display(),
                e
            )))?;
    }
    
    let content = toml::to_string_pretty(config)
        .map_err(|e| RemoteFsError::Configuration(format!(
            "Failed to serialize config: {}",
            e
        )))?;
    
    fs::write(path.as_ref(), content)
        .map_err(|e| RemoteFsError::Configuration(format!(
            "Failed to write config file {}: {}",
            path.as_ref().display(),
            e
        )))
}

/// Get the default configuration file path
pub fn get_default_config_path() -> PathBuf {
    let config_dir = get_config_dir();
    config_dir.join("config.toml")
}

/// Get the configuration directory, creating it if necessary
pub fn get_config_dir() -> PathBuf {
    let home_dir = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let config_dir = home_dir.join(".remotefs");
    
    // Create the directory if it doesn't exist
    let _ = fs::create_dir_all(&config_dir);
    
    config_dir
}

/// Validate a configuration
pub fn validate_config(config: &AgentConfig) -> Result<()> {
    // Validate agent ID
    if config.agent_id.is_empty() {
        return Err(RemoteFsError::Configuration(
            "Agent ID cannot be empty".to_string()
        ));
    }
    
    // Validate relay URL
    if config.relay_url.is_empty() {
        return Err(RemoteFsError::Configuration(
            "Relay URL cannot be empty".to_string()
        ));
    }
    
    if !config.relay_url.starts_with("ws://") && !config.relay_url.starts_with("wss://") {
        return Err(RemoteFsError::Configuration(
            "Relay URL must start with ws:// or wss://".to_string()
        ));
    }
    
    // Validate access configuration
    if config.access.allowed_paths.is_empty() {
        return Err(RemoteFsError::Configuration(
            "At least one allowed path must be specified".to_string()
        ));
    }
    
    // Check that allowed paths exist
    for path in &config.access.allowed_paths {
        if !Path::new(path).exists() {
            return Err(RemoteFsError::Configuration(format!(
                "Allowed path does not exist: {}",
                path
            )));
        }
    }
    
    // Validate file size limit
    if config.access.max_file_size == 0 {
        return Err(RemoteFsError::Configuration(
            "Maximum file size must be greater than 0".to_string()
        ));
    }
    
    // Validate performance settings
    if config.performance.worker_threads == 0 {
        return Err(RemoteFsError::Configuration(
            "Worker threads must be greater than 0".to_string()
        ));
    }
    
    if config.performance.io_buffer_size == 0 {
        return Err(RemoteFsError::Configuration(
            "IO buffer size must be greater than 0".to_string()
        ));
    }
    
    // Validate logging level
    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    if !valid_levels.contains(&config.logging.level.as_str()) {
        return Err(RemoteFsError::Configuration(format!(
            "Invalid log level: {}. Must be one of: {}",
            config.logging.level,
            valid_levels.join(", ")
        )));
    }
    
    Ok(())
}

/// Merge two configurations, with the second one taking precedence
pub fn merge_configs(base: &AgentConfig, overlay: &AgentConfig) -> AgentConfig {
    AgentConfig {
        agent_id: if overlay.agent_id.is_empty() {
            base.agent_id.clone()
        } else {
            overlay.agent_id.clone()
        },
        relay_url: if overlay.relay_url.is_empty() {
            base.relay_url.clone()
        } else {
            overlay.relay_url.clone()
        },
        access: merge_access_configs(&base.access, &overlay.access),
        security: merge_security_configs(&base.security, &overlay.security),
        network: overlay.network.clone(),
        logging: merge_logging_configs(&base.logging, &overlay.logging),
        performance: merge_performance_configs(&base.performance, &overlay.performance),
    }
}

fn merge_access_configs(base: &AccessConfig, overlay: &AccessConfig) -> AccessConfig {
    AccessConfig {
        allowed_paths: if overlay.allowed_paths.is_empty() {
            base.allowed_paths.clone()
        } else {
            overlay.allowed_paths.clone()
        },
        read_only_paths: if overlay.read_only_paths.is_empty() {
            base.read_only_paths.clone()
        } else {
            overlay.read_only_paths.clone()
        },
        denied_paths: if overlay.denied_paths.is_empty() {
            base.denied_paths.clone()
        } else {
            overlay.denied_paths.clone()
        },
        max_file_size: overlay.max_file_size,
        follow_symlinks: overlay.follow_symlinks,
        allowed_extensions: if overlay.allowed_extensions.is_empty() {
            base.allowed_extensions.clone()
        } else {
            overlay.allowed_extensions.clone()
        },
        denied_extensions: if overlay.denied_extensions.is_empty() {
            base.denied_extensions.clone()
        } else {
            overlay.denied_extensions.clone()
        },
    }
}

fn merge_security_configs(base: &SecurityConfig, overlay: &SecurityConfig) -> SecurityConfig {
    SecurityConfig {
        key_file: overlay.key_file.clone(),
        cert_file: overlay.cert_file.clone(),
        enable_tls: overlay.enable_tls,
        verify_certs: overlay.verify_certs,
        session_timeout: overlay.session_timeout,
        enable_auth: overlay.enable_auth,
        allowed_clients: if overlay.allowed_clients.is_empty() {
            base.allowed_clients.clone()
        } else {
            overlay.allowed_clients.clone()
        },
    }
}

fn merge_logging_configs(base: &LoggingConfig, overlay: &LoggingConfig) -> LoggingConfig {
    LoggingConfig {
        level: if overlay.level.is_empty() {
            base.level.clone()
        } else {
            overlay.level.clone()
        },
        format: if overlay.format.is_empty() {
            base.format.clone()
        } else {
            overlay.format.clone()
        },
        file: overlay.file.clone().or_else(|| base.file.clone()),
        max_file_size: overlay.max_file_size,
        max_files: overlay.max_files,
        enable_access_log: overlay.enable_access_log,
        access_log_file: overlay.access_log_file.clone().or_else(|| base.access_log_file.clone()),
    }
}

fn merge_performance_configs(base: &PerformanceConfig, overlay: &PerformanceConfig) -> PerformanceConfig {
    PerformanceConfig {
        worker_threads: if overlay.worker_threads == 0 {
            base.worker_threads
        } else {
            overlay.worker_threads
        },
        io_buffer_size: if overlay.io_buffer_size == 0 {
            base.io_buffer_size
        } else {
            overlay.io_buffer_size
        },
        async_io: overlay.async_io,
        fs_cache_size: overlay.fs_cache_size,
        enable_prefetch: overlay.enable_prefetch,
        prefetch_window: overlay.prefetch_window,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_default_config() {
        let config = create_default_agent_config();
        assert!(!config.agent_id.is_empty());
        assert!(!config.relay_url.is_empty());
        assert!(!config.access.allowed_paths.is_empty());
        assert!(config.performance.worker_threads > 0);
    }

    #[test]
    fn test_validate_config() {
        let mut config = create_default_agent_config();
        
        // Valid config should pass
        assert!(validate_config(&config).is_ok());
        
        // Empty agent ID should fail
        config.agent_id = "".to_string();
        assert!(validate_config(&config).is_err());
        
        config.agent_id = "test-agent".to_string();
        
        // Invalid relay URL should fail
        config.relay_url = "invalid-url".to_string();
        assert!(validate_config(&config).is_err());
        
        config.relay_url = "ws://localhost:8080/ws".to_string();
        
        // Empty allowed paths should fail
        config.access.allowed_paths.clear();
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        
        let original_config = create_default_agent_config();
        
        // Save config
        assert!(save_config_to_file(&original_config, &config_path).is_ok());
        
        // Load config
        let loaded_config = load_config_from_file(&config_path).unwrap();
        
        assert_eq!(original_config.agent_id, loaded_config.agent_id);
        assert_eq!(original_config.relay_url, loaded_config.relay_url);
    }

    #[test]
    fn test_merge_configs() {
        let base = create_default_agent_config();
        let mut overlay = create_default_agent_config();
        overlay.agent_id = "overlay-agent".to_string();
        overlay.relay_url = "ws://overlay:9090/ws".to_string();
        
        let merged = merge_configs(&base, &overlay);
        
        assert_eq!(merged.agent_id, "overlay-agent");
        assert_eq!(merged.relay_url, "ws://overlay:9090/ws");
    }
}
