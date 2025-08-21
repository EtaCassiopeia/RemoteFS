# RemoteFS Agent Configuration

This directory contains example configuration files for the RemoteFS agent.

## Quick Start

1. **Generate a default configuration:**
   ```bash
   remotefs-agent generate-config --output ~/.config/remotefs/agent.toml
   ```

2. **Edit the configuration file:**
   - Set a unique `agent_id`
   - Configure the `relay_url` to point to your relay server
   - Set `allowed_paths` to the directories you want to expose
   - Review security settings

3. **Validate the configuration:**
   ```bash
   remotefs-agent validate-config ~/.config/remotefs/agent.toml
   ```

4. **Start the agent:**
   ```bash
   remotefs-agent --config ~/.config/remotefs/agent.toml
   ```

## Configuration Options

### Command Line Arguments

The agent supports several command-line options:

```bash
remotefs-agent [OPTIONS] [COMMAND]

Options:
  -c, --config <FILE>          Configuration file path
      --agent-id <ID>          Agent ID (overrides config file)
      --relay-url <URL>        Relay server URL (overrides config file)
      --log-level <LEVEL>      Log level (trace, debug, info, warn, error)
  -v, --verbose                Enable verbose logging
  -d, --daemon                 Run in background/daemon mode
  -h, --help                   Print help
  -V, --version                Print version

Commands:
  generate-config              Generate a default configuration file
  validate-config              Validate configuration file
  run                          Run the agent server (default)
  help                         Print this message or the help of the given subcommand(s)
```

### Environment Variables

The agent also supports configuration via environment variables:

- `REMOTEFS_AGENT_CONFIG`: Path to configuration file
- `REMOTEFS_AGENT_ID`: Agent identifier
- `REMOTEFS_RELAY_URL`: Relay server URL
- `REMOTEFS_LOG_LEVEL`: Logging level
- `REMOTEFS_ALLOWED_PATHS`: Comma-separated list of allowed paths
- `REMOTEFS_MAX_FILE_SIZE`: Maximum file size in bytes

### Configuration File

The agent uses TOML format for configuration files. See `agent-config.example.toml` for a fully documented example.

#### Key Sections:

- **`agent_id`**: Unique identifier for this agent instance
- **`relay_url`**: WebSocket URL of the relay server
- **`[access]`**: Access control settings (paths, file size limits, etc.)
- **`[security]`**: Security settings (TLS, authentication, key files)
- **`[network]`**: Network timeouts and connection settings
- **`[performance]`**: Performance tuning options
- **`[logging]`**: Logging configuration

## Security Best Practices

### 1. Access Control
- Only expose necessary directories in `allowed_paths`
- Use `denied_paths` to explicitly block sensitive directories
- Set appropriate `max_file_size` limits
- Consider disabling `follow_symlinks` in production

### 2. Network Security
- Always use `wss://` (secure WebSocket) in production
- Enable TLS with `enable_tls = true`
- Use certificate verification with `verify_certs = true`
- Configure appropriate network timeouts

### 3. Authentication
- Keep `enable_auth = true` (default)
- Protect private key files with appropriate file permissions (600)
- Use strong, unique `agent_id` values
- Consider using client certificates for mutual TLS

### 4. File System Security
- Run the agent with minimal required privileges
- Use `denied_extensions` to block dangerous file types
- Set `max_file_size` to prevent abuse
- Monitor access logs for suspicious activity

## Example Use Cases

### Development Environment
```toml
agent_id = "dev-agent"
relay_url = "ws://localhost:8080/ws"

[access]
allowed_paths = ["/tmp/dev", "/home/user/projects"]

[security]
enable_tls = false
verify_certs = false

[logging]
level = "debug"
```

### Production Environment
```toml
agent_id = "prod-agent-001"
relay_url = "wss://relay.company.com:8080/ws"

[access]
allowed_paths = ["/var/data/shared"]
read_only_paths = ["/var/data/shared/readonly"]
denied_paths = ["/var/data/shared/private"]
follow_symlinks = false
denied_extensions = ["exe", "bat", "sh", "ps1"]

[security]
enable_tls = true
verify_certs = true
key_file = "/etc/remotefs/agent.key"

[logging]
level = "warn"
file = "/var/log/remotefs/agent.log"
enable_access_log = true
access_log_file = "/var/log/remotefs/access.log"
```

### High-Performance Environment
```toml
[performance]
worker_threads = 16
io_buffer_size = 1048576  # 1MB
fs_cache_size = 1024      # 1GB
enable_prefetch = true
prefetch_window = 16

[network]
max_concurrent_connections = 50
```

## Troubleshooting

### Configuration Issues
1. **Invalid TOML syntax**: Use `validate-config` command to check syntax
2. **Path not found**: Ensure `allowed_paths` exist and are accessible
3. **Permission denied**: Check file permissions on key files and directories

### Connection Issues
1. **Cannot connect to relay**: Verify `relay_url` and network connectivity
2. **TLS errors**: Check certificate configuration and validity
3. **Authentication failed**: Verify key files and agent registration

### Performance Issues
1. **Slow file operations**: Increase `io_buffer_size` and `worker_threads`
2. **High memory usage**: Reduce `fs_cache_size`
3. **Network timeouts**: Adjust timeout values in `[network]` section

## Logging

The agent provides comprehensive logging with configurable levels:

- **trace**: Very detailed debugging information
- **debug**: Detailed debugging information  
- **info**: General operational information (default)
- **warn**: Warning messages about potential issues
- **error**: Error messages for failures

Logs can be output to:
- **Console** (default): Logs to stdout/stderr
- **File**: Logs to a file with automatic rotation
- **JSON format**: Machine-readable structured logs

Example logging configuration:
```toml
[logging]
level = "info"
format = "json"
file = "/var/log/remotefs/agent.log"
max_file_size = 100  # MB
max_files = 10
enable_access_log = true
access_log_file = "/var/log/remotefs/access.log"
```
