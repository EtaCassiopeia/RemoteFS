use remotefs_common::{
    config::AgentConfig,
    load_agent_config,
    error::Result,
};
use std::env;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod access;
mod connection;
mod filesystem;
mod server;

use server::AgentServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting RemoteFS Agent...");

    // Load configuration
    let config_path = env::var("REMOTEFS_AGENT_CONFIG").unwrap_or_else(|_| "agent-config.toml".to_string());
    let config = match load_agent_config(&config_path) {
        Ok(cfg) => {
            info!("Loaded configuration from: {}", config_path);
            cfg
        }
        Err(e) => {
            warn!("Failed to load config from {}: {}. Using default configuration.", config_path, e);
            remotefs_common::config_utils::create_default_agent_config()
        }
    };

    info!("Agent ID: {}", config.agent_id);
    info!("Relay URL: {}", config.relay_url);
    info!("Allowed paths: {:?}", config.access.allowed_paths);
    info!("Security settings - TLS: {}, Auth: {}", 
        config.security.enable_tls, config.security.enable_auth);

    // Create and start the agent server
    let server = AgentServer::new(config)?;
    
    if let Err(e) = server.run().await {
        error!("Agent server error: {}", e);
        std::process::exit(1);
    }

    info!("RemoteFS Agent shutdown complete");
    Ok(())
}
