# RemoteFS Agent

A secure, high-performance agent that provides remote access to local filesystems through the RemoteFS relay server. The agent runs on the machine hosting the files and connects to a relay server to enable secure remote filesystem operations.

## Features

- **Secure Access Control** - Fine-grained path-based access control with allow/deny lists
- **WebSocket Transport** - Persistent, efficient connection to relay servers
- **Authentication & Authorization** - TLS encryption and client authentication
- **Performance Monitoring** - Built-in statistics and health monitoring
- **Flexible Configuration** - TOML configuration with CLI overrides
- **Resource Limits** - Configurable file size limits and rate limiting
- **Comprehensive Logging** - Structured logging with rotation and access logs
- **Background Operation** - Daemon mode with graceful shutdown handling

## Quick Start

### Installation

```bash
# Build from source
cargo build --release --bin remotefs-agent

# Or install directly
cargo install --path .
```

### Basic Usage

1. **Generate Configuration**:
   ```bash
   remotefs-agent generate-config --output agent.toml
   ```

2. **Edit Configuration** (see [Configuration](#configuration)):
   ```bash
   nano agent.toml
   ```

3. **Validate Configuration**:
   ```bash
   remotefs-agent validate-config agent.toml
   ```

4. **Run the Agent**:
   ```bash
   # Foreground mode (for testing)
   remotefs-agent --config agent.toml run --foreground
   
   # Background/daemon mode
   remotefs-agent --config agent.toml --daemon
   ```

## Configuration

The agent uses TOML configuration files with the following structure:

### Complete Configuration Example

```toml
# Agent Configuration
agent_id = "my-agent-001"
relay_url = "wss://relay.example.com:8080/ws"

# Access Control
[access]
allowed_paths = ["/home/user/shared", "/opt/data"]
read_only_paths = ["/opt/data/readonly"]
denied_paths = ["/etc", "/root", "/sys", "/proc"]
max_file_size = 104857600  # 100MB
follow_symlinks = false
allowed_extensions = ["txt", "pdf", "jpg", "png"]
denied_extensions = ["exe", "bat", "cmd", "scr"]

# Security Settings
[security]
key_file = "/home/user/.remotefs/agent.key"
cert_file = "/home/user/.remotefs/agent.crt"
enable_tls = true
verify_certs = true
session_timeout = 3600
enable_auth = true
allowed_clients = ["client-1", "client-2"]

# Network Configuration
[network]
connection_timeout = 30
heartbeat_interval = 60
max_message_size = 67108864  # 64MB
reconnect_interval = 5
max_reconnect_attempts = 10

# Logging Configuration
[logging]
level = "info"
format = "json"  # or "text"
file = "/var/log/remotefs/agent.log"
max_file_size = 10  # MB
max_files = 5
enable_access_log = true
access_log_file = "/var/log/remotefs/access.log"

# Performance Settings
[performance]
worker_threads = 4
io_buffer_size = 65536
async_io = true
fs_cache_size = 128
enable_prefetch = true
prefetch_window = 8
```

### Minimal Configuration

```toml
agent_id = "simple-agent"
relay_url = "ws://localhost:8080/ws"

[access]
allowed_paths = ["/home/user/Documents"]
max_file_size = 10485760  # 10MB

[security]
enable_tls = false
enable_auth = false

[logging]
level = "info"
format = "text"
```

## Command Line Interface

### Main Commands

```bash
# Generate default configuration
remotefs-agent generate-config [OPTIONS]
  -o, --output <FILE>    Output file path
  -f, --force           Force overwrite existing file

# Validate configuration
remotefs-agent validate-config [CONFIG_FILE]

# Run the agent (default command)
remotefs-agent [OPTIONS] run
  -c, --config <FILE>       Configuration file path
      --agent-id <ID>       Agent ID (overrides config)
      --relay-url <URL>     Relay server URL (overrides config)
      --log-level <LEVEL>   Log level (trace, debug, info, warn, error)
  -v, --verbose             Enable verbose logging
  -d, --daemon              Run in background/daemon mode
  -f, --foreground          Run in foreground (overrides daemon flag)
```

### Environment Variables

Configuration can be overridden with environment variables:

- `REMOTEFS_AGENT_CONFIG` - Configuration file path
- `REMOTEFS_AGENT_ID` - Agent identifier
- `REMOTEFS_RELAY_URL` - Relay server WebSocket URL
- `REMOTEFS_LOG_LEVEL` - Logging level
- `REMOTEFS_ALLOWED_PATHS` - Comma-separated allowed paths
- `REMOTEFS_MAX_FILE_SIZE` - Maximum file size in bytes

### Examples

```bash
# Run with custom config file
remotefs-agent --config /etc/remotefs/agent.toml

# Run with CLI overrides
remotefs-agent --agent-id "prod-agent-01" --relay-url "wss://relay.prod.com/ws"

# Run with environment variables
export REMOTEFS_AGENT_ID="env-agent"
export REMOTEFS_RELAY_URL="ws://localhost:8080/ws"
remotefs-agent

# Generate config for production
remotefs-agent generate-config --output /etc/remotefs/agent.toml

# Validate production config
remotefs-agent validate-config /etc/remotefs/agent.toml
```

## Security

### Access Control

The agent implements multiple layers of access control:

1. **Path-based Access**: Define allowed, denied, and read-only paths
2. **File Extension Filtering**: Allow or block specific file extensions
3. **File Size Limits**: Prevent access to files exceeding size limits
4. **Symlink Control**: Choose whether to follow symbolic links

### Authentication & Encryption

- **TLS Encryption**: Secure WebSocket connections (WSS)
- **Client Authentication**: Verify client certificates
- **Key Management**: Automatic key generation and rotation
- **Session Management**: Configurable session timeouts

### Best Practices

1. **Minimal Access**: Only allow access to necessary directories
2. **Read-Only Access**: Use read-only paths for sensitive data
3. **File Size Limits**: Set appropriate limits to prevent abuse
4. **Regular Audits**: Monitor access logs for suspicious activity
5. **Key Security**: Protect private key files with proper permissions

## Monitoring & Logging

### Logging Features

- **Structured Logging**: JSON or text format logs
- **Log Rotation**: Automatic log file rotation with size limits
- **Access Logs**: Separate access log for audit trails
- **Multiple Levels**: trace, debug, info, warn, error

### Performance Monitoring

The agent provides built-in performance monitoring:

- **Operation Statistics**: Track filesystem operations and response times
- **Connection Health**: Monitor relay connection status
- **Resource Usage**: Track memory and I/O usage
- **Error Rates**: Monitor and alert on error conditions

### Health Checks

- Automatic connection health monitoring
- Filesystem accessibility checks
- Performance statistics reporting
- Resource cleanup tasks

## Architecture

### Core Components

1. **AgentServer** - Main server orchestrating all components
2. **ConnectionManager** - Manages WebSocket connection to relay
3. **FilesystemHandler** - Handles filesystem operations with access control
4. **AccessControl** - Implements security policies and path validation
5. **ConfigurationManager** - Loads and validates configuration

### Message Flow

```
Client → Relay Server → Agent → Local Filesystem
                    ← Agent ← Local Filesystem
       ← Relay Server ←
```

### Security Model

```
┌─────────────────┐    TLS/WSS    ┌──────────────────┐
│   RemoteFS      │◄─────────────►│   RemoteFS       │
│   Client        │               │   Agent          │
└─────────────────┘               └──────────────────┘
                                           │
                                           ▼
                                  ┌──────────────────┐
                                  │   Local          │
                                  │   Filesystem     │
                                  └──────────────────┘
```

## Deployment

### System Service (systemd)

Create `/etc/systemd/system/remotefs-agent.service`:

```ini
[Unit]
Description=RemoteFS Agent
After=network.target
Wants=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/remotefs-agent --config /etc/remotefs/agent.toml
ExecReload=/bin/kill -HUP $MAINPID
Restart=always
RestartSec=5
User=remotefs
Group=remotefs
WorkingDirectory=/var/lib/remotefs

# Security settings
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=no
ReadWritePaths=/home /opt/data
ReadOnlyPaths=/etc/remotefs

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl enable remotefs-agent
sudo systemctl start remotefs-agent
sudo systemctl status remotefs-agent
```

### Docker Deployment

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin remotefs-agent

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/remotefs-agent /usr/local/bin/
COPY examples/agent-config.toml /etc/remotefs/agent.toml

EXPOSE 8080
USER 1000:1000
VOLUME ["/data", "/config"]

CMD ["remotefs-agent", "--config", "/config/agent.toml"]
```

### Configuration Management

For production deployments:

1. **Configuration Templates**: Use templating for environment-specific configs
2. **Secret Management**: Store keys and certificates securely
3. **Configuration Validation**: Always validate configs before deployment
4. **Backup**: Backup configuration and keys regularly

## Troubleshooting

### Common Issues

1. **Connection Failed**:
   ```bash
   # Check relay server status
   curl -I http://relay-server:8080/health
   
   # Verify network connectivity
   telnet relay-server 8080
   
   # Check agent logs
   journalctl -u remotefs-agent -f
   ```

2. **Access Denied Errors**:
   ```bash
   # Verify path permissions
   ls -la /path/to/allowed/directory
   
   # Check access configuration
   remotefs-agent validate-config
   
   # Review access logs
   tail -f /var/log/remotefs/access.log
   ```

3. **Performance Issues**:
   ```bash
   # Monitor resource usage
   top -p $(pgrep remotefs-agent)
   
   # Check I/O statistics
   iostat -x 1
   
   # Review performance logs
   grep "Performance Report" /var/log/remotefs/agent.log
   ```

### Debug Mode

Run with verbose logging for debugging:

```bash
remotefs-agent --verbose --config agent.toml run --foreground
```

### Log Analysis

Important log patterns to monitor:

- `Connection established` - Successful relay connection
- `Access denied` - Security violations
- `Performance Report` - Regular performance statistics
- `ERROR` - Any error conditions requiring attention

## Development

### Building from Source

```bash
# Clone repository
git clone https://github.com/your-org/remotefs
cd remotefs/remotefs-agent

# Build
cargo build

# Run tests
cargo test

# Build with all features
cargo build --all-features

# Create release build
cargo build --release
```

### Testing

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test '*'

# Run with test coverage
cargo tarpaulin --out html

# Benchmark tests
cargo bench
```

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
