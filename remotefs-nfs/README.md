# RemoteFS macOS NFS Server

A macOS-compatible filesystem mounting solution for RemoteFS that bypasses FUSE limitations by implementing an NFS server that proxies requests to remote RemoteFS agents.

## Overview

Unlike Linux FUSE-based mounting, macOS has limitations with FUSE compatibility. This solution follows the same approach as [ZeroFS](https://github.com/Barre/zerofs) by implementing a local NFS server that:

1. Connects to RemoteFS agents via WebSocket
2. Exposes the remote filesystem via NFS protocol
3. Allows mounting using macOS's built-in NFS client

## Architecture

```
┌─────────────────┐    NFS     ┌──────────────────────┐    WebSocket    ┌─────────────────┐
│  macOS Client   │◄──────────►│  RemoteFS NFS Server │◄───────────────►│  RemoteFS Agent │
│   (Finder, etc) │   TCP 2049 │     (This crate)     │   Encrypted     │   (Remote host) │
└─────────────────┘            └──────────────────────┘                 └─────────────────┘
```

## Features

- **macOS Native**: Works with macOS built-in NFS client
- **Zero FUSE Dependencies**: No compatibility issues with macOS FUSE implementations
- **Full Filesystem Support**: Read, write, create, delete, rename operations
- **Multiple Agents**: Connect to multiple remote agents simultaneously
- **Caching**: Local caching for improved performance
- **Authentication**: Secure connection to remote agents
- **Auto-Reconnection**: Handles network interruptions gracefully

## Installation

### From Source

```bash
cd remotefs-macos
cargo install --path .
```

### Pre-built Binaries

Download from the releases page (when available).

## Usage

### Quick Start

1. **Start the NFS server:**
   ```bash
   remotefs-macos start --agents ws://remote-host:8080
   ```

2. **Mount the filesystem:**
   ```bash
   sudo mkdir -p /mnt/remotefs
   sudo mount -t nfs -o vers=3,tcp,port=2049,mountport=2049 127.0.0.1:/ /mnt/remotefs
   ```

3. **Access your files:**
   ```bash
   ls -la /mnt/remotefs
   ```

4. **Unmount when done:**
   ```bash
   sudo umount /mnt/remotefs
   ```

### Configuration

Create a configuration file:

```bash
remotefs-macos config generate
```

Edit the configuration at `~/.config/remotefs/macos.toml`:

```toml
# NFS server settings
host = "127.0.0.1"
port = 2049

# Remote agents to connect to
agents = [
    "ws://remote-host-1:8080",
    "ws://remote-host-2:8080",
]

# Authentication (optional)
[auth]
enabled = true
token = "your-secure-token"

# Performance tuning
[performance]
cache_enabled = true
cache_size_mb = 512
read_buffer_size = 131072  # 128KB
write_buffer_size = 131072 # 128KB
```

### CLI Commands

#### Server Management

```bash
# Start server with default config
remotefs-macos start

# Start with specific config
remotefs-macos -c /path/to/config.toml start

# Start with CLI overrides
remotefs-macos --host 0.0.0.0 --port 2050 --agents ws://host1:8080,ws://host2:8080 start

# Check server status
remotefs-macos status
```

#### Mount Operations

```bash
# Show mount commands
remotefs-macos mount show

# Mount filesystem (requires sudo)
remotefs-macos mount mount /mnt/remotefs

# Unmount filesystem (requires sudo)
remotefs-macos mount unmount /mnt/remotefs
```

#### Configuration

```bash
# Generate example config
remotefs-macos config generate

# Validate config
remotefs-macos config validate

# Show current config
remotefs-macos config show
```

## Mount Options

### Basic Mount

```bash
sudo mount -t nfs -o vers=3,tcp,port=2049,mountport=2049 127.0.0.1:/ /mnt/remotefs
```

### Performance Optimized Mount

```bash
sudo mount -t nfs -o vers=3,tcp,port=2049,mountport=2049,rsize=1048576,wsize=1048576,async 127.0.0.1:/ /mnt/remotefs
```

### Mount Options Explained

- `vers=3`: Use NFS version 3 (required)
- `tcp`: Use TCP transport (more reliable than UDP)
- `port=2049`: NFS server port
- `mountport=2049`: Mount protocol port
- `rsize=1048576`: Read buffer size (1MB for better throughput)
- `wsize=1048576`: Write buffer size (1MB for better throughput)
- `async`: Asynchronous I/O for better performance
- `hard`: Retry indefinitely on network failures (recommended)
- `soft`: Fail after timeout (use with caution)

## Persistent Mounting

Add to `/etc/fstab` for automatic mounting at boot:

```
127.0.0.1:/ /mnt/remotefs nfs vers=3,tcp,port=2049,mountport=2049,rsize=1048576,wsize=1048576,_netdev 0 0
```

The `_netdev` option ensures mounting waits for network availability.

## Performance

### Benchmarks

Typical performance on modern hardware:

- **Sequential Read**: 200-500 MB/s (depending on network and agent)
- **Sequential Write**: 150-300 MB/s
- **Random I/O**: 10-50 MB/s
- **Latency**: 1-5ms (local network)

### Optimization Tips

1. **Use SSD cache directory**: Set `cache_dir` to SSD path
2. **Increase buffer sizes**: Set read/write buffers to 128KB-1MB
3. **Enable compression**: For slow networks
4. **Multiple agents**: Load balance across multiple remote hosts
5. **Local networking**: Use gigabit+ networking

## Troubleshooting

### Common Issues

#### Mount Permission Denied

```bash
# Ensure server is running
remotefs-macos status

# Check if port is in use
netstat -an | grep 2049

# Try different port
remotefs-macos --port 2050 start
```

#### Connection Refused

```bash
# Check agent connectivity
curl -v ws://remote-host:8080

# Verify firewall settings
# macOS: System Preferences > Security & Privacy > Firewall
```

#### Stale File Handle

```bash
# Force unmount and remount
sudo umount -f /mnt/remotefs
sudo mount -t nfs -o vers=3,tcp 127.0.0.1:/ /mnt/remotefs
```

### Debugging

Enable debug logging:

```bash
remotefs-macos -v start
```

Or set environment variable:

```bash
RUST_LOG=debug remotefs-macos start
```

## Security

### Authentication

RemoteFS supports multiple authentication methods:

- **Token-based**: Shared secret tokens
- **TLS certificates**: Mutual TLS authentication
- **Network isolation**: VPN or private networks

### Encryption

All communication with remote agents is encrypted using:

- **ChaCha20-Poly1305**: Fast, secure symmetric encryption
- **X25519**: Key exchange for forward secrecy
- **HKDF**: Key derivation for multiple keys

### Network Security

- Use `wss://` (WebSocket Secure) for production
- Deploy agents behind VPN or private networks
- Use firewall rules to restrict NFS access

## Comparison with FUSE

| Feature | RemoteFS NFS | FUSE |
|---------|-------------|------|
| macOS Compatibility | ✅ Native | ❌ Limited |
| Performance | ✅ High | ⚠️ Variable |
| Stability | ✅ Stable | ❌ Can crash |
| Permission Model | ✅ Standard | ⚠️ Complex |
| Installation | ✅ No extra deps | ❌ Requires macFUSE |
| Network Filesystems | ✅ Designed for | ⚠️ Local focus |

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## License

MIT OR Apache-2.0

## Credits

Inspired by [ZeroFS](https://github.com/Barre/zerofs) and their excellent NFS-based approach to solving macOS filesystem mounting limitations.
