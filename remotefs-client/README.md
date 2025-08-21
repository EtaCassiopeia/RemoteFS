# RemoteFS Client

A high-performance, async Rust client library for connecting to RemoteFS agents and performing distributed filesystem operations.

## Features

- **Async/Await API** - Built on Tokio for high-performance async operations
- **Connection Pooling** - Manage multiple agent connections with automatic failover
- **Load Balancing** - Multiple strategies including round-robin, weighted, and least-connections
- **Retry Logic** - Configurable retry strategies with exponential backoff
- **WebSocket Transport** - Efficient binary protocol over WebSocket connections
- **Comprehensive Operations** - Full filesystem API (read, write, list, metadata, etc.)
- **Flexible Configuration** - TOML, JSON, and programmatic configuration
- **CLI Tool** - Command-line interface for interactive usage
- **Statistics & Monitoring** - Built-in performance metrics and connection monitoring
- **Error Handling** - Comprehensive error types with retry detection

## Quick Start

### Library Usage

```rust
use remotefs_client::*;
use bytes::Bytes;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
        ],
        client: ClientBehaviorConfig::default(),
        connection: ConnectionConfig::default(),
        auth: None,
        logging: LoggingConfig::default(),
    };

    // Create and initialize client
    let client = RemoteFsClient::new(config)?;
    client.initialize().await?;

    // Perform filesystem operations
    let data = Bytes::from("Hello, RemoteFS!");
    client.write_file("/path/to/file.txt", data).await?;
    
    let content = client.read_file("/path/to/file.txt").await?;
    println!("File content: {}", String::from_utf8_lossy(&content));

    // Shutdown
    client.shutdown().await?;
    Ok(())
}
```

### CLI Usage

```bash
# Write a file
remotefs-client write /remote/path/file.txt --data "Hello, World!"

# Read a file
remotefs-client read /remote/path/file.txt

# List directory
remotefs-client list /remote/path/

# Get file metadata
remotefs-client metadata /remote/path/file.txt

# Create directory
remotefs-client mkdir /remote/path/newdir --mode 755

# Copy file
remotefs-client copy /remote/source.txt /remote/dest.txt

# Move/rename file
remotefs-client move /remote/old.txt /remote/new.txt

# Delete file
remotefs-client delete-file /remote/path/file.txt

# Show client statistics
remotefs-client stats

# Show connection status
remotefs-client status
```

## Configuration

The client supports multiple configuration formats:

### TOML Configuration

```toml
# client.toml
[[agents]]
id = "primary"
url = "ws://localhost:8080"
weight = 2
enabled = true

[client]
operation_timeout_ms = 30000
max_retries = 3

[client.retry_strategy]
type = "exponential"
base_delay_ms = 1000
max_delay_ms = 10000

[connection]
connect_timeout_ms = 10000
heartbeat_interval_ms = 30000
```

### JSON Configuration

```json
{
  "agents": [
    {
      "id": "primary",
      "url": "ws://localhost:8080",
      "weight": 2,
      "enabled": true
    }
  ],
  "client": {
    "operation_timeout_ms": 30000,
    "max_retries": 3,
    "retry_strategy": {
      "type": "exponential",
      "base_delay_ms": 1000,
      "max_delay_ms": 10000
    }
  }
}
```

### Programmatic Configuration

```rust
use remotefs_client::*;

let config = ClientConfig {
    agents: vec![
        AgentConfig {
            id: "agent1".to_string(),
            url: "ws://localhost:8080".to_string(),
            auth: Some(AuthConfig {
                method: AuthMethod::Token,
                credentials: AuthCredentials::Token {
                    token: "your-token".to_string(),
                },
            }),
            weight: 1,
            enabled: true,
        },
    ],
    client: ClientBehaviorConfig {
        operation_timeout_ms: 30000,
        max_retries: 5,
        retry_strategy: RetryStrategy::Exponential {
            base_delay_ms: 1000,
            max_delay_ms: 30000,
        },
        load_balancing: LoadBalancingStrategy::WeightedRoundRobin,
        enable_failover: true,
        read_buffer_size: 8192,
        write_buffer_size: 8192,
    },
    connection: ConnectionConfig {
        connect_timeout_ms: 10000,
        heartbeat_interval_ms: 30000,
        max_message_size: 64 * 1024 * 1024,
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
```

## API Reference

### Core Client

```rust
impl RemoteFsClient {
    // Client lifecycle
    pub fn new(config: ClientConfig) -> ClientResult<Self>;
    pub async fn initialize(&self) -> ClientResult<()>;
    pub async fn shutdown(&self) -> ClientResult<()>;
    
    // File operations
    pub async fn read_file<P: AsRef<Path>>(&self, path: P) -> ClientResult<Bytes>;
    pub async fn read_file_range<P: AsRef<Path>>(&self, path: P, offset: Option<u64>, length: Option<u64>) -> ClientResult<Bytes>;
    pub async fn write_file<P: AsRef<Path>>(&self, path: P, data: Bytes) -> ClientResult<()>;
    pub async fn write_file_at<P: AsRef<Path>>(&self, path: P, data: Bytes, offset: Option<u64>, sync: bool) -> ClientResult<()>;
    
    // Directory operations
    pub async fn list_directory<P: AsRef<Path>>(&self, path: P) -> ClientResult<Vec<DirEntry>>;
    pub async fn create_directory<P: AsRef<Path>>(&self, path: P) -> ClientResult<()>;
    pub async fn create_directory_with_mode<P: AsRef<Path>>(&self, path: P, mode: u32) -> ClientResult<()>;
    pub async fn delete_directory<P: AsRef<Path>>(&self, path: P) -> ClientResult<()>;
    
    // File management
    pub async fn delete_file<P: AsRef<Path>>(&self, path: P) -> ClientResult<()>;
    pub async fn move_path<P: AsRef<Path>>(&self, source: P, destination: P) -> ClientResult<()>;
    pub async fn copy_file<P: AsRef<Path>>(&self, source: P, destination: P) -> ClientResult<()>;
    
    // Metadata
    pub async fn get_metadata<P: AsRef<Path>>(&self, path: P) -> ClientResult<FileMetadata>;
    pub async fn get_metadata_with_options<P: AsRef<Path>>(&self, path: P, follow_symlinks: bool) -> ClientResult<FileMetadata>;
    
    // Monitoring
    pub async fn get_stats(&self) -> ClientStats;
    pub async fn get_connection_status(&self) -> Vec<(String, ConnectionState)>;
}
```

### Configuration Types

```rust
pub struct ClientConfig {
    pub agents: Vec<AgentConfig>,
    pub client: ClientBehaviorConfig,
    pub connection: ConnectionConfig,
    pub auth: Option<AuthConfig>,
    pub logging: LoggingConfig,
}

pub struct AgentConfig {
    pub id: String,
    pub url: String,
    pub auth: Option<AuthConfig>,
    pub weight: u32,
    pub enabled: bool,
}

pub enum RetryStrategy {
    None,
    Linear { delay_ms: u64 },
    Exponential { base_delay_ms: u64, max_delay_ms: u64 },
}

pub enum LoadBalancingStrategy {
    RoundRobin,
    WeightedRoundRobin,
    LeastConnections,
    Random,
}
```

### Error Handling

```rust
pub enum ClientError {
    Connection(String),
    Authentication(String),
    Network(tokio_tungstenite::tungstenite::Error),
    Serialization(bincode::Error),
    Configuration(String),
    Timeout { seconds: u64 },
    AgentUnavailable { message: String },
    InvalidResponse(String),
    RemoteFs(RemoteFsError),
    Io(std::io::Error),
    UrlParse(url::ParseError),
    Internal(String),
}

impl ClientError {
    pub fn is_retryable(&self) -> bool;
    pub fn is_temporary(&self) -> bool;
}
```

## Load Balancing

The client supports multiple load balancing strategies:

- **Round Robin** - Distributes requests evenly across agents
- **Weighted Round Robin** - Uses agent weights for proportional distribution
- **Least Connections** - Routes to the agent with fewest active connections
- **Random** - Randomly selects an agent for each request

## Retry Logic

Configurable retry strategies with automatic retry detection:

- **None** - No retries
- **Linear** - Fixed delay between retries
- **Exponential** - Exponentially increasing delays with maximum cap

Retryable errors include network failures, timeouts, and temporary agent unavailability.

## Connection Management

- **Automatic Reconnection** - Reconnects to agents when connections are lost
- **Health Monitoring** - Tracks connection status and statistics
- **Heartbeats** - Keep-alive messages to maintain connections
- **Connection Pooling** - Efficient reuse of WebSocket connections

## Authentication

Supports multiple authentication methods:

- **None** - No authentication
- **Token** - Bearer token authentication
- **Certificate** - TLS client certificate authentication
- **Username/Password** - Basic authentication

## Examples

See the `examples/` directory for complete usage examples:

- `basic_usage.rs` - Basic client usage and filesystem operations
- `client_config.toml` - Complete configuration example

## Building

```bash
# Build the library
cargo build

# Build the CLI tool
cargo build --bin remotefs-client

# Run tests
cargo test

# Build documentation
cargo doc --open
```

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Submit a pull request

## License

This project is licensed under either of:

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
