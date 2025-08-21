# RemoteFS Relay Server

A high-performance, secure relay server that facilitates communication between RemoteFS clients and agents. The relay server acts as an intermediary, routing messages between clients and agents while providing authentication, session management, and load balancing capabilities.

## Features

- **WebSocket-based Communication** - Efficient, persistent connections using WebSocket protocol
- **Message Routing** - Intelligent routing between clients and agents with load balancing
- **Session Management** - Comprehensive session handling with timeouts and cleanup
- **Authentication & Security** - TLS encryption and token-based authentication
- **Performance Monitoring** - Built-in statistics and health monitoring
- **Scalability** - Support for thousands of concurrent connections
- **Configuration Management** - Flexible TOML-based configuration
- **Graceful Shutdown** - Clean shutdown with connection cleanup

## Architecture

The RemoteFS Relay Server sits between clients and agents:

```
┌─────────────────┐    WSS/WS     ┌──────────────────┐    WSS/WS     ┌─────────────────┐
│   RemoteFS      │◄─────────────►│   RemoteFS       │◄─────────────►│   RemoteFS      │
│   Client        │               │   Relay Server   │               │   Agent         │
└─────────────────┘               └──────────────────┘               └─────────────────┘
```

Key components:

- **Session Manager** - Tracks active client and agent connections
- **Message Router** - Routes messages between clients and agents  
- **Auth Manager** - Handles authentication and authorization
- **Connection Handler** - Manages WebSocket connections

## Quick Start

### Installation

```bash
# Build from source
cargo build --release --bin remotefs-relay

# Or install directly  
cargo install --path .
```

### Basic Usage

1. **Create Configuration**:
   ```bash
   # Use the minimal config for development
   cp examples/minimal-config.toml relay-config.toml
   ```

2. **Edit Configuration** (optional):
   ```bash
   nano relay-config.toml
   ```

3. **Run the Relay Server**:
   ```bash
   # Development mode
   remotefs-relay

   # With custom config
   REMOTEFS_RELAY_CONFIG=relay-config.toml remotefs-relay

   # Production mode with custom config
   remotefs-relay --config /etc/remotefs/relay.toml
   ```

### Environment Variables

- `REMOTEFS_RELAY_CONFIG` - Path to configuration file
- `RUST_LOG` - Log level (trace, debug, info, warn, error)

## Configuration

The relay server uses TOML configuration files. See the `examples/` directory for complete configuration examples.

### Minimal Configuration

```toml
bind_address = "0.0.0.0"
port = 8080

[security]
enable_tls = false
enable_auth = false

[logging]
level = "info"
format = "text"
```

### Production Configuration

```toml
bind_address = "0.0.0.0"
port = 8443
max_connections = 5000

[message_limits]
max_message_size = 134217728  # 128 MB
max_chunk_size = 4194304      # 4 MB
max_dir_entries = 50000

[session]
timeout = 1800                # 30 minutes
max_sessions = 5000
cleanup_interval = 180        # 3 minutes
enable_persistence = true
storage_path = "/var/lib/remotefs/relay/sessions.db"

[storage]
temp_dir = "/var/lib/remotefs/relay/temp"
max_size_gb = 100.0
temp_file_ttl = 3600         # 1 hour
compress = true
cleanup_interval = 1800      # 30 minutes

[security]
key_file = "/etc/ssl/private/remotefs-relay.key"
cert_file = "/etc/ssl/certs/remotefs-relay.crt"
enable_tls = true
verify_certs = false
session_timeout = 1800
enable_auth = true
allowed_clients = []

[network]
connection_timeout = 120
read_timeout = 300           # 5 minutes
write_timeout = 300          # 5 minutes
heartbeat_interval = 30
max_reconnect_attempts = 3
max_concurrent_connections = 50
tcp_keepalive = true
keepalive_interval = 30

[logging]
level = "info"
format = "json"
file = "/var/log/remotefs/relay.log"
max_file_size = 50          # MB
max_files = 30
enable_access_log = true
access_log_file = "/var/log/remotefs/relay-access.log"
```

## API Endpoints

The relay server provides several HTTP endpoints in addition to WebSocket:

### Health Check
```
GET /health
```
Returns: `OK` if the server is healthy.

### Statistics
```
GET /stats  
```
Returns: Plain text statistics about active sessions and message routing.

### WebSocket Endpoint
```
WS /ws
```
Main WebSocket endpoint for client and agent connections.

## Message Flow

1. **Authentication**: Clients and agents connect and authenticate
2. **Session Creation**: Successful authentication creates a managed session
3. **Message Routing**: Messages are routed between authenticated clients and agents
4. **Load Balancing**: Multiple agents can serve requests with automatic load balancing
5. **Session Management**: Sessions are monitored and cleaned up automatically

### Supported Message Types

- **Auth Messages**: `AuthRequest`, `AuthResponse`
- **File Operations**: `ReadFile`, `WriteFile`, `ListDirectory`, etc.
- **Metadata Operations**: `GetMetadata`, `SetMetadata`
- **Directory Operations**: `CreateDirectory`, `RemoveDirectory`
- **Management**: `Ping`, `Pong`, `ConnectionClose`

## Security

### Authentication

The relay server supports token-based authentication:

- Clients and agents authenticate with public keys
- Session tokens are generated for authenticated sessions
- Tokens have configurable expiration times
- Optional client allowlisting for additional security

### TLS Encryption

- Full TLS support for WebSocket connections (WSS)
- Certificate-based encryption
- Configurable certificate verification
- Perfect forward secrecy

### Access Control

- Session-based access control
- Configurable session timeouts
- Connection rate limiting
- Message size limits

## Monitoring & Logging

### Logging Levels

- **trace**: Extremely detailed debugging information
- **debug**: Detailed debugging information  
- **info**: General informational messages (recommended for production)
- **warn**: Warning messages for potential issues
- **error**: Error messages for failures

### Log Formats

- **text**: Human-readable format for development
- **json**: Structured format for log aggregation systems

### Metrics

The relay server tracks:

- Active sessions (clients vs agents)
- Message routing statistics
- Connection statistics
- Authentication statistics
- Error rates and types

Access metrics via:
- `/stats` HTTP endpoint
- Application logs
- Optional external monitoring integration

## Deployment

### System Service (systemd)

Create `/etc/systemd/system/remotefs-relay.service`:

```ini
[Unit]
Description=RemoteFS Relay Server
After=network.target
Wants=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/remotefs-relay --config /etc/remotefs/relay.toml
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=5
User=remotefs
Group=remotefs
WorkingDirectory=/var/lib/remotefs

# Security hardening
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=/var/lib/remotefs /var/log/remotefs

# Network access
IPAddressDeny=any
IPAddressAllow=localhost
IPAddressAllow=10.0.0.0/8
IPAddressAllow=172.16.0.0/12
IPAddressAllow=192.168.0.0/16

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable remotefs-relay
sudo systemctl start remotefs-relay
sudo systemctl status remotefs-relay
```

### Docker Deployment

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin remotefs-relay

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/remotefs-relay /usr/local/bin/
COPY examples/production-config.toml /etc/remotefs/relay.toml

EXPOSE 8080 8443
USER 1000:1000
VOLUME ["/var/lib/remotefs", "/var/log/remotefs", "/etc/remotefs"]

CMD ["remotefs-relay", "--config", "/etc/remotefs/relay.toml"]
```

### Load Balancer Configuration

For high availability, run multiple relay servers behind a load balancer:

#### nginx Configuration
```nginx
upstream remotefs_relay {
    server relay1.example.com:8443;
    server relay2.example.com:8443;
    server relay3.example.com:8443;
}

server {
    listen 443 ssl;
    server_name relay.example.com;
    
    ssl_certificate /etc/ssl/certs/relay.example.com.crt;
    ssl_certificate_key /etc/ssl/private/relay.example.com.key;
    
    location /ws {
        proxy_pass https://remotefs_relay;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # WebSocket specific
        proxy_cache_bypass $http_upgrade;
        proxy_read_timeout 86400;
    }
    
    location /health {
        proxy_pass https://remotefs_relay;
        proxy_set_header Host $host;
    }
    
    location /stats {
        proxy_pass https://remotefs_relay;
        proxy_set_header Host $host;
        # Restrict access to stats
        allow 10.0.0.0/8;
        allow 172.16.0.0/12; 
        allow 192.168.0.0/16;
        deny all;
    }
}
```

## Performance Tuning

### Connection Limits

Adjust based on your hardware and expected load:

```toml
max_connections = 5000           # Total connections
max_concurrent_connections = 50  # Per client limit
```

### Message Limits

Configure based on your use cases:

```toml
[message_limits]
max_message_size = 134217728     # 128 MB for large files
max_chunk_size = 4194304         # 4 MB chunks
max_dir_entries = 50000          # Large directory support
```

### Session Management

Optimize for your session patterns:

```toml
[session]
timeout = 1800                   # 30 minutes
cleanup_interval = 180           # 3 minutes
enable_persistence = true        # For reliability
```

### System Tuning

For high-concurrency deployments:

```bash
# Increase file descriptor limits
echo "remotefs soft nofile 65536" >> /etc/security/limits.conf
echo "remotefs hard nofile 65536" >> /etc/security/limits.conf

# Tune TCP settings
echo "net.core.somaxconn = 65536" >> /etc/sysctl.conf
echo "net.ipv4.tcp_max_syn_backlog = 65536" >> /etc/sysctl.conf
sysctl -p
```

## Troubleshooting

### Common Issues

1. **Connection Refused**:
   ```bash
   # Check if the relay is running
   systemctl status remotefs-relay
   
   # Check port binding
   netstat -tlnp | grep :8080
   
   # Check logs
   journalctl -u remotefs-relay -f
   ```

2. **TLS Certificate Issues**:
   ```bash
   # Verify certificate
   openssl x509 -in /etc/ssl/certs/remotefs-relay.crt -text -noout
   
   # Check certificate chain
   openssl verify /etc/ssl/certs/remotefs-relay.crt
   
   # Test TLS connection
   openssl s_client -connect relay.example.com:8443
   ```

3. **High Memory Usage**:
   ```bash
   # Check session count
   curl http://localhost:8080/stats
   
   # Monitor memory usage
   top -p $(pgrep remotefs-relay)
   
   # Reduce session limits or timeout
   ```

### Debug Mode

Run with debug logging for troubleshooting:

```bash
RUST_LOG=debug remotefs-relay --config relay-config.toml
```

### Log Analysis

Key log patterns to monitor:

- `Authentication request` - Client/agent authentication attempts  
- `Session expired` - Session cleanup events
- `Message routed` - Successful message routing
- `Failed to route` - Routing failures requiring attention
- `WebSocket connection` - Connection events

## Development

### Building

```bash
# Debug build
cargo build

# Release build  
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run
```

### Testing

```bash
# Unit tests
cargo test --lib

# Integration tests  
cargo test --test '*'

# Run specific test
cargo test test_routing_stats

# Test with logging
RUST_LOG=debug cargo test -- --nocapture
```

### Example Usage

See `examples/basic_usage.rs` for a complete working example.

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Add tests for new functionality
5. Run the test suite (`cargo test`)
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

## License

This project is licensed under either of:

- Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
