use remotefs_common::{
    load_relay_config,
    error::Result,
};
use std::env;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod auth;
mod routing;
mod server;
mod session;

use auth::AuthManager;
use server::RelayServer;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting RemoteFS Relay Server...");

    // Load configuration
    let config_path = env::var("REMOTEFS_RELAY_CONFIG").unwrap_or_else(|_| "relay-config.toml".to_string());
    let config = match load_relay_config(&config_path) {
        Ok(cfg) => {
            info!("Loaded configuration from: {}", config_path);
            cfg
        }
        Err(e) => {
            warn!("Failed to load config from {}: {}. Using default configuration.", config_path, e);
            remotefs_common::config_utils::create_default_relay_config()
        }
    };

    // Create authentication manager
    let auth_manager = Arc::new(AuthManager::new(&config));
    info!("Authentication manager initialized (auth enabled: {})", config.security.enable_auth);

    // Create and start the relay server
    let server = RelayServer::new(config.clone(), auth_manager.clone())?;
    
    // Set up graceful shutdown
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.run().await {
            error!("Server error: {}", e);
        }
    });

    // Set up periodic cleanup tasks
    let cleanup_auth_manager = auth_manager.clone();
    let cleanup_handle = tokio::spawn(async move {
        let mut cleanup_interval = tokio::time::interval(tokio::time::Duration::from_secs(300)); // 5 minutes
        loop {
            cleanup_interval.tick().await;
            let removed = cleanup_auth_manager.cleanup_expired_tokens().await;
            if removed > 0 {
                info!("Cleaned up {} expired authentication tokens", removed);
            }
        }
    });

    // Set up stats reporting
    let stats_auth_manager = auth_manager.clone();
    let stats_handle = tokio::spawn(async move {
        let mut stats_interval = tokio::time::interval(tokio::time::Duration::from_secs(600)); // 10 minutes
        loop {
            stats_interval.tick().await;
            let auth_stats = stats_auth_manager.get_auth_stats().await;
            info!(
                "Server stats - Active sessions: {} (clients: {}, agents: {})",
                auth_stats.total_authenticated,
                auth_stats.authenticated_clients,
                auth_stats.authenticated_agents
            );
        }
    });

    info!(
        "RemoteFS Relay Server started on {}:{}",
        config.bind_address, config.port
    );

    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(_) => {
            info!("Received shutdown signal, gracefully shutting down...");
        }
        Err(err) => {
            error!("Unable to listen for shutdown signal: {}", err);
        }
    }

    // Cancel background tasks
    cleanup_handle.abort();
    stats_handle.abort();
    server_handle.abort();

    info!("RemoteFS Relay Server shutdown complete");
    Ok(())
}
