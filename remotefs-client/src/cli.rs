use crate::client::RemoteFsClient;
use crate::config::ClientConfig;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, error};
use bytes::Bytes;

#[derive(Parser)]
#[command(name = "remotefs-client")]
#[command(about = "RemoteFS client for interacting with remote filesystem agents")]
pub struct CliArgs {
    /// Configuration file path
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    
    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
    
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Read a file from the remote filesystem
    Read {
        /// Path to the file to read
        path: String,
        /// Output file path (optional, will print to stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Write data to a file on the remote filesystem
    Write {
        /// Path to the file to write
        path: String,
        /// Input file path (will read from stdin if not provided)
        #[arg(short, long)]
        input: Option<PathBuf>,
        /// Data to write (if not reading from file or stdin)
        #[arg(short, long)]
        data: Option<String>,
    },
    /// List directory contents
    List {
        /// Directory path to list
        path: String,
    },
    /// Get file or directory metadata
    Metadata {
        /// Path to get metadata for
        path: String,
        /// Follow symlinks
        #[arg(short, long)]
        follow_symlinks: bool,
    },
    /// Create a directory
    Mkdir {
        /// Directory path to create
        path: String,
        /// Directory permissions (octal)
        #[arg(short, long, default_value = "755")]
        mode: String,
    },
    /// Delete a file
    DeleteFile {
        /// File path to delete
        path: String,
    },
    /// Delete a directory
    DeleteDir {
        /// Directory path to delete
        path: String,
    },
    /// Move/rename a file or directory
    Move {
        /// Source path
        source: String,
        /// Destination path
        destination: String,
    },
    /// Copy a file
    Copy {
        /// Source path
        source: String,
        /// Destination path
        destination: String,
    },
    /// Show client statistics
    Stats,
    /// Show connection status
    Status,
}

pub async fn run(args: CliArgs) -> Result<()> {
    // Load configuration
    let config = if let Some(config_path) = args.config {
        ClientConfig::from_file(config_path)?
    } else {
        ClientConfig::default()
    };
    
    // Create and initialize client
    let client = RemoteFsClient::new(config)?;
    client.initialize().await?;
    
    info!("Connected to RemoteFS agents");
    
    // Execute command
    match args.command {
        Commands::Read { path, output } => {
            let data = client.read_file(&path).await?;
            
            if let Some(output_path) = output {
                tokio::fs::write(output_path, data).await?;
                info!("File written to {:?}", output);
            } else {
                // Print to stdout
                print!("{}", String::from_utf8_lossy(&data));
            }
        }
        
        Commands::Write { path, input, data } => {
            let content = if let Some(input_path) = input {
                tokio::fs::read(input_path).await?
            } else if let Some(data_str) = data {
                data_str.into_bytes()
            } else {
                // Read from stdin
                use tokio::io::{AsyncReadExt, stdin};
                let mut buffer = Vec::new();
                stdin().read_to_end(&mut buffer).await?;
                buffer
            };
            
            client.write_file(&path, Bytes::from(content)).await?;
            info!("File written successfully");
        }
        
        Commands::List { path } => {
            let entries = client.list_directory(&path).await?;
            
            for entry in entries {
                println!("{}", entry.name);
            }
        }
        
        Commands::Metadata { path, follow_symlinks } => {
            let metadata = client.get_metadata_with_options(&path, follow_symlinks).await?;
            
            println!("Path: {}", path);
            println!("Type: {:?}", metadata.file_type);
            println!("Size: {} bytes", metadata.size);
            println!("Permissions: {:o}", metadata.permissions);
            if let Some(modified) = metadata.modified {
                println!("Modified: {:?}", modified);
            }
            if let Some(accessed) = metadata.accessed {
                println!("Accessed: {:?}", accessed);
            }
        }
        
        Commands::Mkdir { path, mode } => {
            let mode_value = u32::from_str_radix(&mode, 8)
                .map_err(|_| anyhow::anyhow!("Invalid mode: {}", mode))?;
            
            client.create_directory_with_mode(&path, mode_value).await?;
            info!("Directory created successfully");
        }
        
        Commands::DeleteFile { path } => {
            client.delete_file(&path).await?;
            info!("File deleted successfully");
        }
        
        Commands::DeleteDir { path } => {
            client.delete_directory(&path).await?;
            info!("Directory deleted successfully");
        }
        
        Commands::Move { source, destination } => {
            client.move_path(&source, &destination).await?;
            info!("File moved successfully");
        }
        
        Commands::Copy { source, destination } => {
            client.copy_file(&source, &destination).await?;
            info!("File copied successfully");
        }
        
        Commands::Stats => {
            let stats = client.get_stats().await;
            
            println!("Client Statistics:");
            println!("  Total operations: {}", stats.operations_total);
            println!("  Successful operations: {}", stats.operations_successful);
            println!("  Failed operations: {}", stats.operations_failed);
            println!("  Bytes read: {}", stats.bytes_read);
            println!("  Bytes written: {}", stats.bytes_written);
            println!("  Average response time: {:.2}ms", stats.avg_response_time_ms);
            println!("  Active connections: {}", stats.active_connections);
        }
        
        Commands::Status => {
            let statuses = client.get_connection_status().await;
            
            println!("Connection Status:");
            for (agent_id, state) in statuses {
                println!("  {}: {:?}", agent_id, state);
            }
        }
    }
    
    // Shutdown client
    client.shutdown().await?;
    
    Ok(())
}
