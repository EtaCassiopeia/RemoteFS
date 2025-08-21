use remotefs_common::{
    config::{AgentConfig, load_agent_config, save_config},
    config_utils::create_default_agent_config,
    defaults,
    error::{Result, RemoteFsError},
};
use clap::{Parser, Subcommand};
use std::{env, path::PathBuf};
use tracing::{error, info, warn, debug};
use tracing_subscriber::{layer::{SubscriberExt, Layer}, util::SubscriberInitExt, fmt, EnvFilter};
use tracing_appender::{rolling, non_blocking};

mod access;
mod connection;
mod filesystem;
mod server;

use server::AgentServer;

/// RemoteFS Agent - Provides secure remote filesystem access
#[derive(Parser)]
#[command(name = "remotefs-agent")]
#[command(about = "A secure remote filesystem agent")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Agent ID (overrides config file)
    #[arg(long, value_name = "ID")]
    agent_id: Option<String>,

    /// Relay server URL (overrides config file)
    #[arg(long, value_name = "URL")]
    relay_url: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, value_name = "LEVEL")]
    log_level: Option<String>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Run in background/daemon mode
    #[arg(short, long)]
    daemon: bool,

    /// Subcommands
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a default configuration file
    GenerateConfig {
        /// Output file path
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        
        /// Force overwrite existing file
        #[arg(short, long)]
        force: bool,
    },
    /// Validate configuration file
    ValidateConfig {
        /// Configuration file to validate
        #[arg(value_name = "FILE")]
        config_file: Option<PathBuf>,
    },
    /// Run the agent server (default)
    Run {
        /// Run in foreground (overrides daemon flag)
        #[arg(short, long)]
        foreground: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Handle subcommands
    if let Some(ref command) = cli.command {
        match command {
            Commands::GenerateConfig { output, force } => {
                return generate_config_file(output.clone(), *force).await;
            }
            Commands::ValidateConfig { config_file } => {
                return validate_config_file(config_file.clone(), cli.config.clone()).await;
            }
            Commands::Run { foreground: _ } => {
                // Continue to main agent logic
            }
        }
    }

    // Determine config file path
    let config_path = determine_config_path(cli.config.clone());
    
    // Load and validate configuration
    let config = load_and_merge_config(&config_path, &cli).await?;
    
    // Validate configuration
    validate_agent_config(&config)?;
    
    // Initialize logging based on configuration
    initialize_logging(&config, cli.verbose)?;
    
    info!("Starting RemoteFS Agent v{}", env!("CARGO_PKG_VERSION"));
    debug!("Configuration loaded from: {}", config_path.display());
    
    // Log configuration summary
    log_config_summary(&config);
    
    // Create directories if needed
    ensure_directories_exist(&config)?;
    
    // Validate access to key files
    validate_key_files(&config)?;
    
    // Create and start the agent server
    let server = AgentServer::new(config)?;
    
    if let Err(e) = server.run().await {
        error!("Agent server error: {}", e);
        std::process::exit(1);
    }

    info!("RemoteFS Agent shutdown complete");
    Ok(())
}

/// Determine the configuration file path
fn determine_config_path(cli_path: Option<PathBuf>) -> PathBuf {
    cli_path
        .or_else(|| env::var("REMOTEFS_AGENT_CONFIG").ok().map(PathBuf::from))
        .unwrap_or_else(|| defaults::agent_config_path())
}

/// Load configuration and merge with CLI overrides
async fn load_and_merge_config(config_path: &PathBuf, cli: &Cli) -> Result<AgentConfig> {
    let mut config = if config_path.exists() {
        match load_agent_config(config_path) {
            Ok(cfg) => {
                info!("Loaded configuration from: {}", config_path.display());
                cfg
            }
            Err(e) => {
                warn!(
                    "Failed to load config from {}: {}. Using default configuration.", 
                    config_path.display(), 
                    e
                );
                create_default_agent_config()
            }
        }
    } else {
        warn!(
            "Configuration file {} not found. Using default configuration.", 
            config_path.display()
        );
        create_default_agent_config()
    };
    
    // Apply CLI overrides
    if let Some(agent_id) = &cli.agent_id {
        config.agent_id = agent_id.clone();
    }
    
    if let Some(relay_url) = &cli.relay_url {
        config.relay_url = relay_url.clone();
    }
    
    if let Some(log_level) = &cli.log_level {
        config.logging.level = log_level.clone();
    }
    
    if cli.verbose {
        config.logging.level = "debug".to_string();
    }
    
    // Apply environment variable overrides
    apply_env_overrides(&mut config)?;
    
    Ok(config)
}

/// Apply environment variable overrides to configuration
fn apply_env_overrides(config: &mut AgentConfig) -> Result<()> {
    if let Ok(agent_id) = env::var("REMOTEFS_AGENT_ID") {
        config.agent_id = agent_id;
    }
    
    if let Ok(relay_url) = env::var("REMOTEFS_RELAY_URL") {
        config.relay_url = relay_url;
    }
    
    if let Ok(log_level) = env::var("REMOTEFS_LOG_LEVEL") {
        config.logging.level = log_level;
    }
    
    if let Ok(allowed_paths) = env::var("REMOTEFS_ALLOWED_PATHS") {
        config.access.allowed_paths = allowed_paths
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
    }
    
    if let Ok(max_file_size) = env::var("REMOTEFS_MAX_FILE_SIZE") {
        config.access.max_file_size = max_file_size.parse().map_err(|e| {
            RemoteFsError::Configuration(format!("Invalid REMOTEFS_MAX_FILE_SIZE: {}", e))
        })?;
    }
    
    Ok(())
}

/// Initialize logging based on configuration
fn initialize_logging(config: &AgentConfig, verbose: bool) -> Result<()> {
    let log_level = if verbose {
        "debug"
    } else {
        &config.logging.level
    };
    
    let env_filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(log_level))
        .map_err(|e| RemoteFsError::Configuration(format!("Invalid log level: {}", e)))?;
    
    let subscriber = tracing_subscriber::registry().with(env_filter);
    
    match (&config.logging.file, &config.logging.format) {
        (Some(log_file), format) => {
            // File logging with rotation
            let file_appender = rolling::daily(log_file.parent().unwrap_or(&PathBuf::from(".")), 
                                             log_file.file_name().unwrap_or(std::ffi::OsStr::new("agent.log")));
            let (non_blocking, _guard) = non_blocking(file_appender);
            
            let fmt_layer = if format == "json" {
                fmt::layer()
                    .with_writer(non_blocking)
                    .json()
                    .boxed()
            } else {
                fmt::layer()
                    .with_writer(non_blocking)
                    .boxed()
            };
            
            subscriber.with(fmt_layer).init();
        }
        (None, format) => {
            // Console logging
            let fmt_layer = if format == "json" {
                fmt::layer().json().boxed()
            } else {
                fmt::layer().boxed()
            };
            
            subscriber.with(fmt_layer).init();
        }
    }
    
    Ok(())
}

/// Validate agent configuration
fn validate_agent_config(config: &AgentConfig) -> Result<()> {
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
    
    // Validate URL format
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
    
    // Validate paths exist and are accessible
    for path in &config.access.allowed_paths {
        let path_buf = PathBuf::from(path);
        if !path_buf.exists() {
            warn!("Allowed path does not exist: {}", path);
        } else if !path_buf.is_dir() {
            warn!("Allowed path is not a directory: {}", path);
        }
    }
    
    // Validate log level
    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    if !valid_levels.contains(&config.logging.level.as_str()) {
        return Err(RemoteFsError::Configuration(
            format!("Invalid log level '{}'. Must be one of: {}", 
                   config.logging.level, valid_levels.join(", "))
        ));
    }
    
    Ok(())
}

/// Log configuration summary
fn log_config_summary(config: &AgentConfig) {
    info!("Agent ID: {}", config.agent_id);
    info!("Relay URL: {}", config.relay_url);
    info!("Allowed paths: {:?}", config.access.allowed_paths);
    
    if !config.access.denied_paths.is_empty() {
        info!("Denied paths: {:?}", config.access.denied_paths);
    }
    
    if !config.access.read_only_paths.is_empty() {
        info!("Read-only paths: {:?}", config.access.read_only_paths);
    }
    
    info!("Security settings - TLS: {}, Auth: {}", 
          config.security.enable_tls, config.security.enable_auth);
    info!("Max file size: {} bytes", config.access.max_file_size);
    info!("Worker threads: {}", config.performance.worker_threads);
    
    debug!("Network timeout: {}s", config.network.connection_timeout);
    debug!("Heartbeat interval: {}s", config.network.heartbeat_interval);
    debug!("Log level: {}", config.logging.level);
}

/// Ensure required directories exist
fn ensure_directories_exist(config: &AgentConfig) -> Result<()> {
    // Create parent directories for key files
    if let Some(parent) = config.security.key_file.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                RemoteFsError::Configuration(format!(
                    "Failed to create key file directory {}: {}",
                    parent.display(), e
                ))
            })?;
            info!("Created key file directory: {}", parent.display());
        }
    }
    
    // Create parent directories for log files
    if let Some(log_file) = &config.logging.file {
        if let Some(parent) = log_file.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    RemoteFsError::Configuration(format!(
                        "Failed to create log directory {}: {}",
                        parent.display(), e
                    ))
                })?;
                info!("Created log directory: {}", parent.display());
            }
        }
    }
    
    Ok(())
}

/// Validate access to key files
fn validate_key_files(config: &AgentConfig) -> Result<()> {
    // Check if key file exists and is readable
    if config.security.enable_auth {
        if !config.security.key_file.exists() {
            warn!("Private key file does not exist: {}. Keys will be generated on first run.", 
                  config.security.key_file.display());
        } else {
            // Try to read the key file to ensure it's accessible
            std::fs::read(&config.security.key_file).map_err(|e| {
                RemoteFsError::Configuration(format!(
                    "Cannot read private key file {}: {}",
                    config.security.key_file.display(), e
                ))
            })?;
            debug!("Private key file is accessible: {}", config.security.key_file.display());
        }
    }
    
    Ok(())
}

/// Generate a default configuration file
async fn generate_config_file(output: Option<PathBuf>, force: bool) -> Result<()> {
    let output_path = output.unwrap_or_else(|| defaults::agent_config_path());
    
    if output_path.exists() && !force {
        return Err(RemoteFsError::Configuration(format!(
            "Configuration file already exists: {}. Use --force to overwrite.",
            output_path.display()
        )));
    }
    
    let default_config = create_default_agent_config();
    save_config(&default_config, &output_path)?;
    
    println!("Generated default configuration file: {}", output_path.display());
    println!("");
    println!("IMPORTANT: Please review and edit the configuration file before running the agent:");
    println!("  - Update the agent_id to a unique identifier");
    println!("  - Set the correct relay_url for your relay server");
    println!("  - Configure allowed_paths for the directories you want to expose");
    println!("  - Update security settings including key file paths");
    println!("");
    
    Ok(())
}

/// Validate a configuration file
async fn validate_config_file(config_file: Option<PathBuf>, cli_config: Option<PathBuf>) -> Result<()> {
    let config_path = config_file.or(cli_config).unwrap_or_else(|| defaults::agent_config_path());
    
    if !config_path.exists() {
        return Err(RemoteFsError::Configuration(format!(
            "Configuration file does not exist: {}",
            config_path.display()
        )));
    }
    
    match load_agent_config(&config_path) {
        Ok(config) => {
            println!("✅ Configuration file is valid: {}", config_path.display());
            
            match validate_agent_config(&config) {
                Ok(()) => {
                    println!("✅ Configuration validation passed");
                    println!("");
                    println!("Configuration summary:");
                    println!("  Agent ID: {}", config.agent_id);
                    println!("  Relay URL: {}", config.relay_url);
                    println!("  Allowed paths: {:?}", config.access.allowed_paths);
                    println!("  Security - TLS: {}, Auth: {}", 
                            config.security.enable_tls, config.security.enable_auth);
                }
                Err(e) => {
                    println!("❌ Configuration validation failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to parse configuration file: {}", e);
            std::process::exit(1);
        }
    }
    
    Ok(())
}
