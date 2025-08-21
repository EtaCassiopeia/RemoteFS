use std::sync::Arc;
use remotefs_common::{
    config_utils::create_default_relay_config,
    error::Result,
};
use remotefs_relay::{server::RelayServer, auth::AuthManager};
use tracing::{info, Level};

/// Basic example of creating and running a RemoteFS relay server
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging for the example
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    println!("🚀 RemoteFS Relay Server Basic Usage Example");
    println!("=============================================");

    // Example 1: Create a default configuration
    println!("\n📝 Creating default configuration...");
    let mut config = create_default_relay_config();
    
    // Customize the configuration for this example
    config.bind_address = "127.0.0.1".to_string(); // Bind to localhost only for example
    config.port = 8080;
    
    // Use simpler security settings for the example
    config.security.enable_tls = false;
    config.security.enable_auth = false;
    
    // Enable verbose logging for the example
    config.logging.level = "debug".to_string();
    config.logging.format = "text".to_string();
    config.logging.file = None; // Log to console only
    
    println!("✅ Configuration created:");
    println!("   Bind address: {}", config.bind_address);
    println!("   Port: {}", config.port);
    println!("   Max connections: {}", config.max_connections);
    println!("   Security: TLS={}, Auth={}", config.security.enable_tls, config.security.enable_auth);

    // Example 2: Create authentication manager
    println!("\n🔐 Creating authentication manager...");
    let auth_manager = Arc::new(AuthManager::new(&config));
    println!("✅ Authentication manager created (auth enabled: {})", config.security.enable_auth);

    // Example 3: Create and configure the relay server
    println!("\n🔧 Creating relay server...");
    let server = RelayServer::new(config.clone(), auth_manager.clone())?;
    println!("✅ Relay server created successfully");

    // Example 4: Show server configuration summary
    println!("\n📊 Server Configuration Summary:");
    println!("   Listening on: {}:{}", config.bind_address, config.port);
    println!("   Max connections: {}", config.max_connections);
    println!("   Session timeout: {} seconds", config.session.timeout);
    println!("   Max message size: {} bytes", config.message_limits.max_message_size);
    
    // Example 5: Start the server (in a real application, this would run indefinitely)
    println!("\n🌟 Starting relay server...");
    println!("   Note: In this example, the server will run for 10 seconds then shutdown");
    println!("   In production, the server would run indefinitely until stopped");
    println!("   You can test it by connecting with clients and agents on ws://127.0.0.1:8080/ws");
    
    // Create a timeout for the example
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("❌ Relay server error: {}", e);
        }
    });
    
    // Let the server run for a few seconds in this example
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    
    // Shutdown the server
    server_handle.abort();
    
    println!("\n🏁 Example completed!");
    println!("   The relay server would normally continue running until stopped with Ctrl+C");
    println!("   Use the configuration files in examples/ for different deployment scenarios");
    
    Ok(())
}

/// Helper function to demonstrate configuration customization for development
fn customize_config_for_development(mut config: remotefs_common::config::RelayConfig) -> remotefs_common::config::RelayConfig {
    // Development-friendly settings
    config.bind_address = "127.0.0.1".to_string(); // Localhost only
    config.port = 8080;
    config.max_connections = 100; // Lower limit for development
    
    config.security.enable_tls = false;
    config.security.enable_auth = false;
    
    config.logging.level = "debug".to_string();
    config.logging.format = "text".to_string();
    config.logging.file = None;
    
    config.session.timeout = 7200; // Longer sessions for development (2 hours)
    config.session.max_sessions = 50;
    
    config
}

/// Helper function to demonstrate configuration for production
fn customize_config_for_production(mut config: remotefs_common::config::RelayConfig) -> remotefs_common::config::RelayConfig {
    // Production settings
    config.bind_address = "0.0.0.0".to_string(); // All interfaces
    config.port = 8443; // HTTPS port for WSS
    config.max_connections = 5000; // High capacity
    
    config.security.enable_tls = true;
    config.security.enable_auth = true;
    config.security.session_timeout = 1800; // 30 minutes
    
    config.logging.level = "info".to_string();
    config.logging.format = "json".to_string();
    config.logging.file = Some(std::path::PathBuf::from("/var/log/remotefs/relay.log"));
    config.logging.enable_access_log = true;
    config.logging.access_log_file = Some(std::path::PathBuf::from("/var/log/remotefs/relay-access.log"));
    
    config.session.timeout = 1800; // 30 minutes
    config.session.max_sessions = 5000;
    config.session.cleanup_interval = 180; // 3 minutes
    
    config.message_limits.max_message_size = 134217728; // 128 MB
    
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_basic_relay_creation() {
        let config = create_default_relay_config();
        let auth_manager = Arc::new(AuthManager::new(&config));
        let result = RelayServer::new(config, auth_manager);
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_config_customization() {
        let config = create_default_relay_config();
        let dev_config = customize_config_for_development(config.clone());
        let prod_config = customize_config_for_production(config);
        
        // Development config should have relaxed settings
        assert_eq!(dev_config.bind_address, "127.0.0.1");
        assert!(!dev_config.security.enable_tls);
        assert!(!dev_config.security.enable_auth);
        assert_eq!(dev_config.logging.format, "text");
        
        // Production config should have strict settings
        assert_eq!(prod_config.bind_address, "0.0.0.0");
        assert!(prod_config.security.enable_tls);
        assert!(prod_config.security.enable_auth);
        assert_eq!(prod_config.logging.format, "json");
        assert_eq!(prod_config.port, 8443);
    }
    
    #[test]
    fn test_default_config_values() {
        let config = create_default_relay_config();
        
        assert_eq!(config.port, 8080);
        assert!(config.max_connections > 0);
        assert!(config.session.timeout > 0);
        assert!(config.message_limits.max_message_size > 0);
        assert!(config.security.enable_tls); // Should be true by default
        assert!(config.security.enable_auth); // Should be true by default
    }
}
