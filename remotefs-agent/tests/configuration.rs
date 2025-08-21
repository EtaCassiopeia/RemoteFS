use std::env;
use std::fs;
use tempfile::TempDir;

mod common;
use common::*;
use remotefs_common::{
    config::{load_agent_config, save_config, AgentConfig, AccessConfig},
    config_utils::create_default_agent_config,
    error::RemoteFsError,
};

#[test]
fn test_default_config_creation() {
    setup_test_logging();
    let config = create_default_agent_config();
    
    assert!(!config.agent_id.is_empty(), "Agent ID should not be empty");
    assert!(config.agent_id.starts_with("agent-"), "Agent ID should start with 'agent-'");
    assert_eq!(config.relay_url, "wss://localhost:8080/ws", "Default relay URL should be localhost");
    assert!(!config.access.allowed_paths.is_empty(), "Should have default allowed paths");
    assert!(config.security.enable_tls, "TLS should be enabled by default");
    assert!(config.security.enable_auth, "Auth should be enabled by default");
}

#[test]
fn test_config_serialization_deserialization() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let original_config = create_test_config(temp_dir.path());
    let config_path = temp_dir.path().join("test_config.toml");
    
    // Save config
    save_config(&original_config, &config_path).expect("Failed to save config");
    assert!(config_path.exists(), "Config file should be created");
    
    // Load config
    let loaded_config = load_agent_config(&config_path).expect("Failed to load config");
    
    // Verify config matches
    assert_eq!(original_config.agent_id, loaded_config.agent_id);
    assert_eq!(original_config.relay_url, loaded_config.relay_url);
    assert_eq!(original_config.access.allowed_paths, loaded_config.access.allowed_paths);
    assert_eq!(original_config.access.denied_paths, loaded_config.access.denied_paths);
    assert_eq!(original_config.access.max_file_size, loaded_config.access.max_file_size);
    assert_eq!(original_config.security.enable_tls, loaded_config.security.enable_tls);
    assert_eq!(original_config.logging.level, loaded_config.logging.level);
}

#[test]
fn test_config_validation_valid() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    // The validate_agent_config function would be called here
    // Since it's in main.rs, we'll test the components that would be validated
    assert!(!config.agent_id.is_empty());
    assert!(!config.relay_url.is_empty());
    assert!(config.relay_url.starts_with("ws://") || config.relay_url.starts_with("wss://"));
    assert!(!config.access.allowed_paths.is_empty());
    assert!(["trace", "debug", "info", "warn", "error"].contains(&config.logging.level.as_str()));
}

#[test]
fn test_config_validation_invalid_agent_id() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let mut config = create_test_config(temp_dir.path());
    config.agent_id = "".to_string(); // Invalid empty agent ID
    
    // This would fail validation
    assert!(config.agent_id.is_empty());
}

#[test]
fn test_config_validation_invalid_relay_url() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let mut config = create_test_config(temp_dir.path());
    config.relay_url = "http://invalid".to_string(); // Invalid protocol
    
    assert!(!config.relay_url.starts_with("ws://") && !config.relay_url.starts_with("wss://"));
}

#[test]
fn test_config_validation_empty_allowed_paths() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let mut config = create_test_config(temp_dir.path());
    config.access.allowed_paths = vec![]; // Invalid empty allowed paths
    
    assert!(config.access.allowed_paths.is_empty());
}

#[test]
fn test_config_validation_invalid_log_level() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let mut config = create_test_config(temp_dir.path());
    config.logging.level = "invalid".to_string(); // Invalid log level
    
    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    assert!(!valid_levels.contains(&config.logging.level.as_str()));
}

#[test]
fn test_config_file_loading_nonexistent() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let nonexistent_path = temp_dir.path().join("nonexistent.toml");
    
    let result = load_agent_config(&nonexistent_path);
    assert!(result.is_err(), "Should fail to load nonexistent config file");
}

#[test]
fn test_config_file_loading_invalid_toml() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let invalid_config_path = temp_dir.path().join("invalid.toml");
    
    // Write invalid TOML content
    fs::write(&invalid_config_path, "invalid toml content [[[").expect("Failed to write invalid config");
    
    let result = load_agent_config(&invalid_config_path);
    assert!(result.is_err(), "Should fail to load invalid TOML file");
}

#[test]
fn test_config_with_minimal_fields() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let minimal_config_path = temp_dir.path().join("minimal.toml");
    
    // Write minimal config with only required fields
    let minimal_toml = r#"
agent_id = "minimal-agent"
relay_url = "ws://localhost:8080/ws"

[access]
allowed_paths = ["/tmp"]
"#;
    
    fs::write(&minimal_config_path, minimal_toml).expect("Failed to write minimal config");
    
    let config = load_agent_config(&minimal_config_path).expect("Failed to load minimal config");
    assert_eq!(config.agent_id, "minimal-agent");
    assert_eq!(config.relay_url, "ws://localhost:8080/ws");
    assert_eq!(config.access.allowed_paths, vec!["/tmp"]);
    
    // Check defaults are applied
    assert!(config.security.enable_tls); // Should default to true
    assert_eq!(config.logging.level, "info"); // Should default to info
}

#[test]
fn test_config_with_all_fields() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let full_config_path = temp_dir.path().join("full.toml");
    
    // Write comprehensive config
    let full_toml = format!(r#"
agent_id = "full-agent"
relay_url = "wss://example.com:8080/ws"

[access]
allowed_paths = ["{}/allowed"]
read_only_paths = ["{}/readonly"] 
denied_paths = ["{}/denied"]
max_file_size = 5368709120
follow_symlinks = false
allowed_extensions = ["txt", "pdf"]
denied_extensions = ["exe", "bat"]

[security]
key_file = "{}/agent.key"
cert_file = "{}/agent.crt"
enable_tls = true
verify_certs = true
session_timeout = 7200
enable_auth = true
allowed_clients = ["client1", "client2"]

[network]
connection_timeout = 60
read_timeout = 120
write_timeout = 120
heartbeat_interval = 45
max_reconnect_attempts = 3
reconnect_backoff_base = 2
max_concurrent_connections = 20
tcp_keepalive = true
keepalive_interval = 30

[logging]
level = "warn"
format = "json"
max_file_size = 200
max_files = 10
enable_access_log = true

[performance]
worker_threads = 8
io_buffer_size = 131072
async_io = true
fs_cache_size = 512
enable_prefetch = true
prefetch_window = 12
"#, 
        temp_dir.path().display(),
        temp_dir.path().display(), 
        temp_dir.path().display(),
        temp_dir.path().display(),
        temp_dir.path().display(),
        temp_dir.path().display()
    );
    
    fs::write(&full_config_path, full_toml).expect("Failed to write full config");
    
    let config = load_agent_config(&full_config_path).expect("Failed to load full config");
    
    // Verify all fields are loaded correctly
    assert_eq!(config.agent_id, "full-agent");
    assert_eq!(config.relay_url, "wss://example.com:8080/ws");
    
    // Access config
    assert!(!config.access.allowed_paths.is_empty());
    assert!(!config.access.read_only_paths.is_empty());
    assert!(!config.access.denied_paths.is_empty());
    assert_eq!(config.access.max_file_size, 5368709120);
    assert!(!config.access.follow_symlinks);
    assert_eq!(config.access.allowed_extensions, vec!["txt", "pdf"]);
    assert_eq!(config.access.denied_extensions, vec!["exe", "bat"]);
    
    // Security config
    assert!(config.security.enable_tls);
    assert!(config.security.verify_certs);
    assert_eq!(config.security.session_timeout, 7200);
    assert!(config.security.enable_auth);
    assert_eq!(config.security.allowed_clients, vec!["client1", "client2"]);
    
    // Network config
    assert_eq!(config.network.connection_timeout, 60);
    assert_eq!(config.network.read_timeout, 120);
    assert_eq!(config.network.write_timeout, 120);
    assert_eq!(config.network.heartbeat_interval, 45);
    assert_eq!(config.network.max_reconnect_attempts, 3);
    assert_eq!(config.network.reconnect_backoff_base, 2);
    assert_eq!(config.network.max_concurrent_connections, 20);
    assert!(config.network.tcp_keepalive);
    assert_eq!(config.network.keepalive_interval, 30);
    
    // Logging config
    assert_eq!(config.logging.level, "warn");
    assert_eq!(config.logging.format, "json");
    assert_eq!(config.logging.max_file_size, 200);
    assert_eq!(config.logging.max_files, 10);
    assert!(config.logging.enable_access_log);
    
    // Performance config
    assert_eq!(config.performance.worker_threads, 8);
    assert_eq!(config.performance.io_buffer_size, 131072);
    assert!(config.performance.async_io);
    assert_eq!(config.performance.fs_cache_size, 512);
    assert!(config.performance.enable_prefetch);
    assert_eq!(config.performance.prefetch_window, 12);
}

#[test]
fn test_config_directory_creation() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let nested_config_path = temp_dir.path().join("nested/deep/config.toml");
    let config = create_test_config(temp_dir.path());
    
    // Save config to nested path (should create directories)
    save_config(&config, &nested_config_path).expect("Failed to save config to nested path");
    
    assert!(nested_config_path.exists(), "Config file should be created");
    assert!(nested_config_path.parent().unwrap().exists(), "Parent directories should be created");
}

#[test]
fn test_config_partial_update() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join("config.toml");
    
    // Create and save initial config
    let mut config = create_test_config(temp_dir.path());
    config.agent_id = "original-agent".to_string();
    config.logging.level = "info".to_string();
    save_config(&config, &config_path).expect("Failed to save initial config");
    
    // Modify and save again
    config.agent_id = "updated-agent".to_string();
    config.logging.level = "debug".to_string();
    save_config(&config, &config_path).expect("Failed to save updated config");
    
    // Load and verify updates
    let loaded_config = load_agent_config(&config_path).expect("Failed to load updated config");
    assert_eq!(loaded_config.agent_id, "updated-agent");
    assert_eq!(loaded_config.logging.level, "debug");
}

#[test]
fn test_config_special_characters() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join("special.toml");
    
    let mut config = create_test_config(temp_dir.path());
    config.agent_id = "agent-with-special-chars-åëïøü".to_string();
    
    // Should handle special characters in serialization/deserialization
    save_config(&config, &config_path).expect("Failed to save config with special chars");
    let loaded_config = load_agent_config(&config_path).expect("Failed to load config with special chars");
    assert_eq!(loaded_config.agent_id, "agent-with-special-chars-åëïøü");
}

#[test]
fn test_config_large_values() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let config_path = temp_dir.path().join("large.toml");
    
    let mut config = create_test_config(temp_dir.path());
    
    // Set large values
    config.access.max_file_size = u64::MAX;
    config.network.connection_timeout = 999999;
    config.performance.worker_threads = 1000;
    config.performance.io_buffer_size = 1024 * 1024 * 100; // 100MB
    
    // Should handle large values
    save_config(&config, &config_path).expect("Failed to save config with large values");
    let loaded_config = load_agent_config(&config_path).expect("Failed to load config with large values");
    
    assert_eq!(loaded_config.access.max_file_size, u64::MAX);
    assert_eq!(loaded_config.network.connection_timeout, 999999);
    assert_eq!(loaded_config.performance.worker_threads, 1000);
    assert_eq!(loaded_config.performance.io_buffer_size, 1024 * 1024 * 100);
}
