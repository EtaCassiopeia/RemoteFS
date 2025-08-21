//! Basic usage example for the RemoteFS client library
//!
//! This example demonstrates how to:
//! - Configure the RemoteFS client
//! - Connect to remote agents
//! - Perform basic filesystem operations
//! - Handle errors and cleanup

use remotefs_client::*;
use bytes::Bytes;
use std::path::Path;
use tokio;
use tracing::{info, error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::init();

    // Create client configuration
    let config = ClientConfig {
        agents: vec![
            AgentConfig {
                id: "agent1".to_string(),
                url: "ws://localhost:8080".to_string(),
                auth: None,
                weight: 1,
                enabled: true,
            },
            AgentConfig {
                id: "agent2".to_string(),
                url: "ws://localhost:8081".to_string(),
                auth: None,
                weight: 2,
                enabled: true,
            },
        ],
        client: ClientBehaviorConfig {
            operation_timeout_ms: 30000,
            max_retries: 3,
            retry_strategy: RetryStrategy::Exponential {
                base_delay_ms: 1000,
                max_delay_ms: 10000,
            },
            load_balancing: LoadBalancingStrategy::WeightedRoundRobin,
            enable_failover: true,
            read_buffer_size: 8192,
            write_buffer_size: 8192,
        },
        connection: ConnectionConfig {
            connect_timeout_ms: 10000,
            heartbeat_interval_ms: 30000,
            max_message_size: 64 * 1024 * 1024, // 64MB
            enable_compression: false,
            reconnection: ReconnectionConfig {
                enabled: true,
                max_attempts: 5,
                base_delay_ms: 1000,
                max_delay_ms: 30000,
                backoff_multiplier: 2.0,
            },
        },
        auth: None,
        logging: LoggingConfig::default(),
    };

    // Create and initialize the client
    let client = RemoteFsClient::new(config)?;
    client.initialize().await?;
    
    info!("RemoteFS client initialized successfully");

    // Basic filesystem operations
    match perform_filesystem_operations(&client).await {
        Ok(_) => info!("All filesystem operations completed successfully"),
        Err(e) => error!("Filesystem operations failed: {}", e),
    }

    // Get client statistics
    let stats = client.get_stats().await;
    info!("Client Statistics: {:?}", stats);

    // Get connection status
    let status = client.get_connection_status().await;
    info!("Connection Status: {:?}", status);

    // Shutdown the client
    client.shutdown().await?;
    info!("RemoteFS client shutdown completed");

    Ok(())
}

async fn perform_filesystem_operations(client: &RemoteFsClient) -> Result<(), ClientError> {
    // Test file path
    let test_file = "/tmp/remotefs_test.txt";
    let test_dir = "/tmp/remotefs_test_dir";
    
    // Write a file
    let content = Bytes::from("Hello, RemoteFS! This is a test file.");
    info!("Writing test file: {}", test_file);
    client.write_file(test_file, content.clone()).await?;

    // Read the file back
    info!("Reading test file: {}", test_file);
    let read_content = client.read_file(test_file).await?;
    assert_eq!(content, read_content);
    info!("File content verification successful");

    // Get file metadata
    info!("Getting file metadata: {}", test_file);
    let metadata = client.get_metadata(test_file).await?;
    info!("File metadata: size={}, permissions={:o}", metadata.size, metadata.permissions);

    // Create a directory
    info!("Creating directory: {}", test_dir);
    client.create_directory(test_dir).await?;

    // List directory contents (parent directory)
    info!("Listing directory contents: /tmp");
    let entries = client.list_directory("/tmp").await?;
    info!("Found {} entries in /tmp", entries.len());

    // Copy the file
    let copy_path = "/tmp/remotefs_test_copy.txt";
    info!("Copying file {} to {}", test_file, copy_path);
    client.copy_file(test_file, copy_path).await?;

    // Move/rename the copy
    let moved_path = "/tmp/remotefs_test_moved.txt";
    info!("Moving file {} to {}", copy_path, moved_path);
    client.move_path(copy_path, moved_path).await?;

    // Clean up - delete files and directory
    info!("Cleaning up test files and directory");
    client.delete_file(test_file).await?;
    client.delete_file(moved_path).await?;
    client.delete_directory(test_dir).await?;

    info!("Cleanup completed successfully");
    Ok(())
}

// Example with error handling
async fn example_with_error_handling(client: &RemoteFsClient) {
    match client.read_file("/nonexistent/file.txt").await {
        Ok(content) => {
            info!("File content: {} bytes", content.len());
        }
        Err(ClientError::RemoteFs(remote_error)) => {
            error!("Remote filesystem error: {}", remote_error);
        }
        Err(ClientError::Connection(msg)) => {
            error!("Connection error: {}", msg);
        }
        Err(ClientError::Timeout { seconds }) => {
            error!("Operation timed out after {} seconds", seconds);
        }
        Err(e) => {
            error!("Other error: {}", e);
        }
    }
}
