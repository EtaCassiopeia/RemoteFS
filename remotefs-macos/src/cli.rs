use crate::{MacOSConfig, RemoteNfsServer, Result};
use clap::{Parser, Subcommand};
use remotefs_client::{Client, ClientError, ClientConfig, AgentConfig, ClientBehaviorConfig, ConnectionConfig, ReconnectionConfig, AuthConfig, AuthMethod, AuthCredentials, LoggingConfig, RetryStrategy, LoadBalancingStrategy};
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "remotefs-macos")]
#[command(about = "RemoteFS macOS NFS Server - Mount remote filesystems via NFS protocol")]
#[command(version)]
pub struct Cli {
    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    
    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,
    
    /// Override the NFS server host
    #[arg(long)]
    pub host: Option<String>,
    
    /// Override the NFS server port
    #[arg(long)]
    pub port: Option<u16>,
    
    /// Override agent endpoints (comma-separated)
    #[arg(long)]
    pub agents: Option<String>,
    
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the NFS server
    Start,
    /// Generate example configuration file
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Mount operations
    Mount {
        #[command(subcommand)]
        action: MountAction,
    },
    /// Check server status
    Status,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Generate example configuration
    Generate,
    /// Validate configuration
    Validate {
        /// Path to configuration file
        path: Option<PathBuf>,
    },
    /// Show current configuration
    Show,
}

#[derive(Subcommand)]
pub enum MountAction {
    /// Show mount command
    Show {
        /// Mount point directory
        #[arg(default_value = "/mnt/remotefs")]
        mount_point: String,
    },
    /// Mount the filesystem (requires sudo)
    Mount {
        /// Mount point directory
        #[arg(default_value = "/mnt/remotefs")]
        mount_point: String,
    },
    /// Unmount the filesystem (requires sudo)
    Unmount {
        /// Mount point directory
        #[arg(default_value = "/mnt/remotefs")]
        mount_point: String,
    },
}

impl Cli {
    pub async fn run(&self) -> Result<()> {
        // Initialize logging
        self.setup_logging();
        
        match &self.command {
            Some(Commands::Start) => self.start_server().await,
            Some(Commands::Config { action }) => self.handle_config(action),
            Some(Commands::Mount { action }) => self.handle_mount(action).await,
            Some(Commands::Status) => self.check_status().await,
            None => self.start_server().await, // Default action
        }
    }
    
    fn setup_logging(&self) {
        let log_level = if self.verbose {
            "debug"
        } else {
            "info"
        };
        
        let filter = format!("remotefs_macos={},remotefs_client={},remotefs_common={}", log_level, log_level, log_level);
        
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter))
            )
            .init();
    }
    
    async fn start_server(&self) -> Result<()> {
        info!("Starting RemoteFS macOS NFS Server");
        
        // Load configuration
        let mut config = self.load_config()?;
        
        // Apply CLI overrides
        self.apply_overrides(&mut config);
        
        // Validate configuration
        config.validate()?;
        
        info!("Configuration loaded and validated");
        
        // Create RemoteFS client
        let client_config = self.create_client_config(&config)?;
        let client = Client::new(client_config)
            .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to create client: {}", e)
            ))?;
        
        // Initialize client (connects to agents)
        info!("Connecting to RemoteFS agents...");
        client.initialize().await
            .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to connect to agents: {}", e)
            ))?;
        info!("Successfully connected to agents");
        
        // Create and initialize NFS server
        let mut server = RemoteNfsServer::new(config);
        server.initialize(client).await?;
        
        // Start server (monitoring is done internally)
        info!("Starting NFS server");
        server.start().await
    }
    
    fn load_config(&self) -> Result<MacOSConfig> {
        if let Some(config_path) = &self.config {
            info!("Loading configuration from {}", config_path.display());
            MacOSConfig::from_file(config_path)
        } else {
            info!("Loading default configuration");
            Ok(MacOSConfig::load_or_default())
        }
    }
    
    fn apply_overrides(&self, config: &mut MacOSConfig) {
        if let Some(ref host) = self.host {
            config.host = host.clone();
        }
        
        if let Some(port) = self.port {
            config.port = port;
        }
        
        if let Some(ref agents) = self.agents {
            config.agents = agents.split(',').map(|s| s.trim().to_string()).collect();
        }
    }
    
    fn create_client_config(&self, config: &MacOSConfig) -> Result<ClientConfig> {
        // Convert agent URLs to AgentConfig structs
        let agents: Vec<AgentConfig> = config.agents.iter().enumerate().map(|(i, url)| {
            AgentConfig {
                id: format!("agent-{}", i),
                url: url.clone(),
                auth: if config.auth.enabled && config.auth.token.is_some() {
                    Some(AuthConfig {
                        method: AuthMethod::Token,
                        credentials: AuthCredentials::Token {
                            token: config.auth.token.as_ref().unwrap().clone(),
                        },
                    })
                } else {
                    None
                },
                weight: 1,
                enabled: true,
            }
        }).collect();
        
        let client_config = ClientConfig {
            agents,
            client: ClientBehaviorConfig {
                operation_timeout_ms: config.request_timeout * 1000,
                max_retries: 3,
                retry_strategy: RetryStrategy::Exponential {
                    base_delay_ms: 1000,
                    max_delay_ms: 30000,
                },
                load_balancing: LoadBalancingStrategy::RoundRobin,
                enable_failover: true,
                read_buffer_size: config.performance.read_buffer_size,
                write_buffer_size: config.performance.write_buffer_size,
            },
            connection: ConnectionConfig {
                connect_timeout_ms: config.connection_timeout * 1000,
                heartbeat_interval_ms: 30000,
                max_message_size: 64 * 1024 * 1024, // 64MB
                enable_compression: config.performance.compression_enabled,
                reconnection: ReconnectionConfig {
                    enabled: true,
                    max_attempts: 5,
                    base_delay_ms: 1000,
                    max_delay_ms: 30000,
                    backoff_multiplier: 2.0,
                },
            },
            auth: None, // Auth is handled per-agent
            logging: LoggingConfig {
                level: if self.verbose { "debug" } else { "info" }.to_string(),
                format: "human".to_string(),
                file: None,
                enable_connection_logs: self.verbose,
                enable_performance_logs: self.verbose,
            },
        };
        
        Ok(client_config)
    }
    
    fn handle_config(&self, action: &ConfigAction) -> Result<()> {
        match action {
            ConfigAction::Generate => {
                MacOSConfig::create_example_config()?;
                println!("Example configuration file created");
                Ok(())
            }
            ConfigAction::Validate { path } => {
                let config_path = path.as_ref()
                    .cloned()
                    .unwrap_or_else(|| MacOSConfig::default_config_path());
                    
                let config = MacOSConfig::from_file(&config_path)?;
                config.validate()?;
                
                println!("Configuration is valid: {}", config_path.display());
                Ok(())
            }
            ConfigAction::Show => {
                let config = self.load_config()?;
                let toml_str = config.to_toml()?;
                println!("{}", toml_str);
                Ok(())
            }
        }
    }
    
    async fn handle_mount(&self, action: &MountAction) -> Result<()> {
        let config = self.load_config()?;
        
        match action {
            MountAction::Show { mount_point } => {
                println!("To mount RemoteFS using NFS:");
                println!();
                println!("1. Create mount point:");
                println!("   sudo mkdir -p {}", mount_point);
                println!();
                println!("2. Mount with NFS:");
                println!("   sudo mount -t nfs -o vers=3,tcp,port={},mountport={} {}:/ {}", 
                         config.port, config.port, config.host, mount_point);
                println!();
                println!("3. To unmount:");
                println!("   sudo umount {}", mount_point);
                println!();
                println!("For better performance, add these options:");
                println!("   -o vers=3,tcp,port={},mountport={},rsize=1048576,wsize=1048576,async", 
                         config.port, config.port);
                Ok(())
            }
            MountAction::Mount { mount_point } => {
                self.mount_filesystem(&config, mount_point).await
            }
            MountAction::Unmount { mount_point } => {
                self.unmount_filesystem(mount_point).await
            }
        }
    }
    
    async fn mount_filesystem(&self, config: &MacOSConfig, mount_point: &str) -> Result<()> {
        use std::process::Command;
        
        info!("Mounting RemoteFS at {}", mount_point);
        
        // Create mount point
        let mkdir_output = Command::new("sudo")
            .args(&["mkdir", "-p", mount_point])
            .output()
            .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to create mount point: {}", e)
            ))?;
            
        if !mkdir_output.status.success() {
            let stderr = String::from_utf8_lossy(&mkdir_output.stderr);
            return Err(remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to create mount point: {}", stderr)
            ));
        }
        
        // Mount filesystem
        let mount_opts = format!("vers=3,tcp,port={},mountport={},rsize=1048576,wsize=1048576,async", 
                          config.port, config.port);
        let host_path = format!("{}:/", config.host);
        
        let mount_output = Command::new("sudo")
            .args(&["mount", "-t", "nfs", "-o", &mount_opts, &host_path, mount_point])
            .output()
            .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to execute mount command: {}", e)
            ))?;
            
        if mount_output.status.success() {
            println!("Successfully mounted RemoteFS at {}", mount_point);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&mount_output.stderr);
            Err(remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to mount: {}", stderr)
            ))
        }
    }
    
    async fn unmount_filesystem(&self, mount_point: &str) -> Result<()> {
        use std::process::Command;
        
        info!("Unmounting RemoteFS from {}", mount_point);
        
        let output = Command::new("sudo")
            .args(&["umount", mount_point])
            .output()
            .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to execute umount command: {}", e)
            ))?;
            
        if output.status.success() {
            println!("Successfully unmounted RemoteFS from {}", mount_point);
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Try force unmount
            warn!("Normal unmount failed, trying force unmount: {}", stderr);
            
            let force_output = Command::new("sudo")
                .args(&["umount", "-f", mount_point])
                .output()
                .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                    format!("Failed to execute force umount command: {}", e)
                ))?;
                
            if force_output.status.success() {
                println!("Successfully force unmounted RemoteFS from {}", mount_point);
                Ok(())
            } else {
                let force_stderr = String::from_utf8_lossy(&force_output.stderr);
                Err(remotefs_common::error::RemoteFsError::Internal(
                    format!("Failed to unmount: {}", force_stderr)
                ))
            }
        }
    }
    
    async fn check_status(&self) -> Result<()> {
        let config = self.load_config()?;
        
        println!("RemoteFS macOS NFS Server Status");
        println!("================================");
        println!();
        
        // Check if NFS port is listening
        use std::net::{TcpStream, SocketAddr};
        use std::time::Duration;
        
        let addr: SocketAddr = format!("{}:{}", config.host, config.port)
            .parse()
            .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                format!("Invalid address: {}", e)
            ))?;
            
        match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
            Ok(_) => {
                println!("✓ NFS server is running on {}:{}", config.host, config.port);
            }
            Err(_) => {
                println!("✗ NFS server is not running on {}:{}", config.host, config.port);
            }
        }
        
        // Check mount points
        use std::process::Command;
        let output = Command::new("mount")
            .output()
            .map_err(|e| remotefs_common::error::RemoteFsError::Internal(
                format!("Failed to check mounts: {}", e)
            ))?;
            
        let mount_output = String::from_utf8_lossy(&output.stdout);
        let remotefs_mounts: Vec<&str> = mount_output
            .lines()
            .filter(|line| line.contains(&format!("{}:{}", config.host, config.port)))
            .collect();
            
        if remotefs_mounts.is_empty() {
            println!("✗ No RemoteFS mounts found");
        } else {
            println!("✓ Active RemoteFS mounts:");
            for mount in remotefs_mounts {
                println!("  {}", mount);
            }
        }
        
        println!();
        println!("Configuration:");
        println!("  Host: {}", config.host);
        println!("  Port: {}", config.port);
        println!("  Agents: {}", config.agents.join(", "));
        
        Ok(())
    }
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();
    cli.run().await
}
