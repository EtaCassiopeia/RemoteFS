# RemoteFS FUSE

RemoteFS FUSE provides FUSE (Filesystem in Userspace) integration for RemoteFS, allowing remote filesystems to be mounted as local directories.

**Note: FUSE support is currently designed for Linux systems. macOS support is experimental due to FUSE library compatibility issues.**

## Features

- **Seamless Integration**: Mount remote filesystems as if they were local directories
- **Multiple Mounts**: Support for multiple simultaneous mount points
- **Configurable Caching**: Flexible attribute and data caching options
- **Load Balancing**: Automatic distribution across multiple agents
- **Failover Support**: Automatic failover to backup agents
- **Permissions Management**: Fine-grained file permission control
- **Configuration-driven**: Easy setup through TOML configuration files

## Requirements

### System Requirements
- Linux operating system (kernel 2.6 or later)
- FUSE kernel module (usually included in modern distributions)
- Root privileges or proper FUSE permissions

### Installing FUSE Dependencies

#### Ubuntu/Debian
```bash
sudo apt-get install fuse3 libfuse3-dev
```

#### CentOS/RHEL/Fedora
```bash
sudo yum install fuse3 fuse3-devel
# or for newer systems
sudo dnf install fuse3 fuse3-devel
```

#### Arch Linux
```bash
sudo pacman -S fuse3
```

## Installation

### From Source
```bash
git clone https://github.com/your-org/remotefs
cd remotefs
cargo build --release --package remotefs-fuse
```

### Using Cargo
```bash
cargo install remotefs-fuse
```

## Configuration

RemoteFS FUSE uses TOML configuration files. See the `examples/fuse/` directory for sample configurations.

### Basic Configuration Structure

```toml
# Client configuration
[client]
client_id = "my-fuse-client"

[[client.agents]]
id = "agent1"
url = "ws://localhost:8080"
enabled = true

# Mount points
[[mounts]]
mount_path = "/mnt/remotefs"
remote_path = "/"
agent_id = "agent1"

# FUSE options
[fuse_options]
fsname = "remotefs"
auto_unmount = true

# Cache settings
[cache]
attr_cache = true
attr_timeout = 10
```

### Configuration Options

#### Client Configuration
- `client_id`: Unique identifier for this FUSE client
- `agents`: List of available RemoteFS agents
- `connection`: Connection settings (timeouts, retries, etc.)
- `logging`: Logging configuration

#### Mount Configuration
- `mount_path`: Local directory to mount to (must exist)
- `remote_path`: Remote path to mount from agent
- `agent_id`: ID of the agent to connect to
- `options`: Mount-specific FUSE options (optional)

#### FUSE Options
- `uid`/`gid`: User and group IDs for mounted files
- `allow_other`: Allow other users to access the mount
- `allow_root`: Allow root to access the mount
- `read_only`: Mount as read-only
- `auto_cache`: Enable automatic caching
- `auto_unmount`: Automatically unmount on exit
- `default_permissions`: Use default permission checking

#### Cache Configuration
- `attr_cache`: Enable attribute caching
- `attr_timeout`: Attribute cache timeout (seconds)
- `entry_timeout`: Directory entry cache timeout (seconds)
- `negative_timeout`: Negative lookup cache timeout (seconds)
- `max_entries`: Maximum number of cached entries
- `write_through`: Enable write-through caching
- `write_cache_size`: Write cache size in bytes

## Usage

### Command Line Interface

Mount a remote filesystem:
```bash
remotefs-mount --config /path/to/config.toml
```

Mount with specific log level:
```bash
remotefs-mount --config config.toml --log-level debug
```

Unmount (from another terminal):
```bash
fusermount -u /mnt/remotefs
```

### Programmatic Usage

```rust
use remotefs_fuse::{FuseConfig, mount_filesystem};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = FuseConfig::load_from_file("config.toml")?;
    
    // Mount filesystem
    let mount_handles = mount_filesystem(config).await?;
    
    // Keep running until interrupted
    tokio::signal::ctrl_c().await?;
    
    // Cleanup is automatic with auto_unmount option
    Ok(())
}
```

## Examples

### Example 1: Single Mount Point
See `examples/fuse/simple_config.toml` for a basic configuration with a single mount point suitable for development and testing.

### Example 2: Multiple Mount Points
See `examples/fuse/fuse_config.toml` for a comprehensive configuration with multiple mount points, different agents, and advanced options.

### Example 3: Read-only Mount
```toml
[[mounts]]
mount_path = "/mnt/readonly"
remote_path = "/shared/readonly"
agent_id = "agent1"

[mounts.options]
read_only = true
allow_other = true
```

### Example 4: High-performance Mount
```toml
[cache]
attr_cache = true
attr_timeout = 60
entry_timeout = 60
write_through = false
write_cache_size = 16777216  # 16MB

[fuse_options]
auto_cache = true
```

## Troubleshooting

### Common Issues

1. **Permission Denied**: Make sure you have proper permissions to mount FUSE filesystems:
   ```bash
   # Add user to fuse group
   sudo usermod -a -G fuse $USER
   # Logout and login again
   ```

2. **Mount Point Busy**: The mount point directory is in use:
   ```bash
   # Check what's using the mount point
   lsof /mnt/remotefs
   # Force unmount if necessary
   sudo fusermount -uz /mnt/remotefs
   ```

3. **Connection Issues**: Agent is not responding:
   - Verify agent is running and accessible
   - Check network connectivity
   - Review agent logs and client configuration

4. **Performance Issues**:
   - Increase cache timeouts for better performance
   - Adjust `write_cache_size` based on workload
   - Consider disabling `write_through` for write-heavy workloads

### Logging

Enable debug logging for troubleshooting:
```toml
[client.logging]
level = "DEBUG"
file_path = "/var/log/remotefs-fuse.log"
```

Or use environment variables:
```bash
RUST_LOG=debug remotefs-mount --config config.toml
```

### Performance Monitoring

Monitor FUSE statistics:
```bash
# View mount information
cat /proc/mounts | grep remotefs

# View FUSE statistics (if available)
cat /sys/fs/fuse/connections/*/statistics
```

## Security Considerations

1. **Network Security**: Use encrypted connections (WSS) for production deployments
2. **File Permissions**: Properly configure `uid`, `gid`, and permission options
3. **Access Control**: Use `allow_other` and `allow_root` options carefully
4. **Credential Management**: Store credentials securely, avoid plain text in configs

## Limitations

1. **Platform Support**: Currently optimized for Linux systems
2. **Network Dependency**: Requires stable network connection to agents
3. **Performance**: Network latency affects file operation performance
4. **Advanced Features**: Some advanced FUSE features may not be supported

## Contributing

Contributions are welcome! Please see the main project's contributing guidelines.

## License

This project is licensed under the MIT OR Apache-2.0 license. See LICENSE files in the project root for details.

## Related Projects

- [RemoteFS Client](../remotefs-client) - Core client library
- [RemoteFS Agent](../remotefs-agent) - File system agent
- [RemoteFS Common](../remotefs-common) - Shared utilities and types
