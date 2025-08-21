use std::path::PathBuf;
use remotefs_agent::{AgentServer, create_default_agent_config, save_config_to_file};
use remotefs_common::config::AgentConfig;
use remotefs_common::error::Result;

/// Basic example of creating and running a RemoteFS agent
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging for the example
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🚀 RemoteFS Agent Basic Usage Example");
    println!("=====================================");

    // Example 1: Create a default configuration
    println!("\n📝 Creating default configuration...");
    let mut config = create_default_agent_config();
    
    // Customize the configuration for this example
    config.agent_id = "example-agent".to_string();
    config.relay_url = "ws://localhost:8080/ws".to_string();
    
    // Set up simple access control for this example
    config.access.allowed_paths = vec![
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("Documents")
            .to_string_lossy()
            .to_string(),
    ];
    
    // Use simpler security settings for the example
    config.security.enable_tls = false;
    config.security.enable_auth = false;
    
    // Enable verbose logging for the example
    config.logging.level = "debug".to_string();
    config.logging.format = "text".to_string();
    config.logging.file = None; // Log to console only
    
    println!("✅ Configuration created:");
    println!("   Agent ID: {}", config.agent_id);
    println!("   Relay URL: {}", config.relay_url);
    println!("   Allowed paths: {:?}", config.access.allowed_paths);
    println!("   Security: TLS={}, Auth={}", config.security.enable_tls, config.security.enable_auth);

    // Example 2: Save configuration to file
    println!("\n💾 Saving configuration to file...");
    let config_path = std::env::temp_dir().join("example-agent.toml");
    save_config_to_file(&config, &config_path)?;
    println!("✅ Configuration saved to: {}", config_path.display());

    // Example 3: Create and configure the agent server
    println!("\n🔧 Creating agent server...");
    let server = AgentServer::new(config)?;
    println!("✅ Agent server created successfully");

    // Example 4: Get initial status
    println!("\n📊 Getting agent status...");
    let status = server.get_status().await;
    println!("✅ Agent Status:");
    println!("   Agent ID: {}", status.agent_id);
    println!("   Connected: {}", status.connected);
    println!("   Uptime: {} seconds", status.uptime_seconds);
    println!("   Active filesystem operations: {}", status.filesystem_stats.active_operations);

    // Example 5: Start the agent (in a real application, this would run indefinitely)
    println!("\n🌟 Starting agent server...");
    println!("   Note: In this example, the server will run for 5 seconds then shutdown");
    println!("   In production, the server would run indefinitely until stopped");
    
    // Create a timeout for the example
    let server_task = tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("❌ Agent server error: {}", e);
        }
    });
    
    // Let the server run for a few seconds in this example
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    
    println!("\n🏁 Example completed!");
    println!("   The agent would normally continue running until stopped with Ctrl+C");
    
    // In a real application, you would typically have:
    // server.run().await?;
    
    Ok(())
}

/// Helper function to demonstrate configuration customization
fn customize_config_for_development(mut config: AgentConfig) -> AgentConfig {
    // Development-friendly settings
    config.security.enable_tls = false;
    config.security.enable_auth = false;
    config.logging.level = "debug".to_string();
    config.logging.format = "text".to_string();
    config.logging.file = None;
    
    // More permissive access for development
    config.access.max_file_size = 50 * 1024 * 1024; // 50MB
    config.access.follow_symlinks = true;
    config.access.allowed_extensions.clear(); // Allow all extensions
    
    config
}

/// Helper function to demonstrate configuration for production
fn customize_config_for_production(mut config: AgentConfig) -> AgentConfig {
    // Production security settings
    config.security.enable_tls = true;
    config.security.enable_auth = true;
    config.security.session_timeout = 1800; // 30 minutes
    
    // Production logging
    config.logging.level = "info".to_string();
    config.logging.format = "json".to_string();
    config.logging.file = Some(PathBuf::from("/var/log/remotefs/agent.log"));
    config.logging.enable_access_log = true;
    config.logging.access_log_file = Some(PathBuf::from("/var/log/remotefs/access.log"));
    
    // Restrictive access control
    config.access.max_file_size = 10 * 1024 * 1024; // 10MB
    config.access.follow_symlinks = false;
    config.access.denied_extensions = vec![
        "exe".to_string(), "bat".to_string(), "cmd".to_string(),
        "sh".to_string(), "py".to_string(), "js".to_string(),
    ];
    
    // Performance tuning
    config.performance.worker_threads = num_cpus::get();
    config.performance.fs_cache_size = 256;
    config.performance.enable_prefetch = true;
    
    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_basic_agent_creation() {
        let config = create_default_agent_config();
        let result = AgentServer::new(config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_config_customization() {
        let config = create_default_agent_config();
        let dev_config = customize_config_for_development(config.clone());
        let prod_config = customize_config_for_production(config);

        // Development config should have relaxed security
        assert!(!dev_config.security.enable_tls);
        assert!(!dev_config.security.enable_auth);

        // Production config should have strict security
        assert!(prod_config.security.enable_tls);
        assert!(prod_config.security.enable_auth);
        assert_eq!(prod_config.logging.format, "json");
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");
        
        let config = create_default_agent_config();
        
        // Save configuration
        let save_result = save_config_to_file(&config, &config_path);
        assert!(save_result.is_ok());
        
        // Verify file was created
        assert!(config_path.exists());
        
        // Verify file is not empty
        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(!content.is_empty());
        assert!(content.contains("agent_id"));
        assert!(content.contains("relay_url"));
    }
}
