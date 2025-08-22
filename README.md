# RemoteFS - Distributed Secure Remote File System

RemoteFS is a comprehensive, production-ready distributed file system that enables secure remote file access through intelligent relay servers. It provides multiple client interfaces including **NFS filesystem mounts** (cross-platform), WebSocket clients, and programmatic APIs for transparent access to remote directories with end-to-end encryption.

> **🎉 NFS Migration Complete**: RemoteFS has migrated from FUSE to NFS for better cross-platform compatibility. NFS provides native support on both Linux and macOS without requiring additional dependencies. See [MIGRATION_GUIDE.md](MIGRATION_GUIDE.md) for migration details.

## Architecture

RemoteFS consists of four main components working together to provide seamless, secure file access:

- **Client Library**: Core WebSocket client with load balancing and connection pooling
- **NFS Integration**: Cross-platform NFS v3 server for transparent filesystem mounting
- **Agent**: Secure file system server running on remote machines
- **Relay Server**: Intelligent load balancer and message router with service discovery

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   NFS Mount     │    │  RemoteFS       │    │  RemoteFS       │    │  RemoteFS       │
│   /mnt/remote   │◄──►│   NFS Server    │◄──►│   Relay         │◄──►│   Agent         │
└─────────────────┘    │   (Linux/macOS) │    │   Server        │    │   Server        │
                       └─────────────────┘    └─────────────────┘    └─────────────────┘
                                │                        │                        │
                                ▼                        ▼                        ▼
                       ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
                       │ NFS v3 Protocol │    │ Service         │    │ Local File      │
                       │ Cross-Platform  │    │ Discovery       │    │ System Access   │
                       │ No Dependencies │    │ & Health Checks │    │ with Security   │
                       └─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Features

- **End-to-end encryption** using ChaCha20-Poly1305
- **Secure key exchange** using X25519 ECDH
- **NFS v3 filesystem** for transparent cross-platform access (Linux & macOS)
- **Native OS integration** - no additional dependencies required
- **Local caching** for improved performance
- **Access control** on the remote agent
- **Connection resilience** with automatic reconnection
- **Cross-platform** support with unified codebase

## Installation

### Prerequisites

- **Rust 1.70+** for building from source
- **Linux or macOS** (Windows support planned)
- **NFS client tools** for filesystem mounting (see platform-specific instructions below)

#### Linux Prerequisites
```bash
# Ubuntu/Debian - Install NFS client
sudo apt-get install nfs-common

# CentOS/RHEL - Install NFS utilities
sudo yum install nfs-utils

# Fedora - Install NFS utilities  
sudo dnf install nfs-utils

# Arch Linux - Install NFS utilities
sudo pacman -S nfs-utils
```

#### macOS Prerequisites
```bash
# macOS has built-in NFS client support - no installation required!
# NFS client tools are included with the operating system
echo "No additional packages needed for macOS NFS support"
```

### Build from Source

```bash
# Clone repository
git clone https://github.com/your-org/remotefs
cd remotefs

# Build all components
cargo build --release --workspace

# Install binaries (optional)
cargo install --path remotefs-client
cargo install --path remotefs-agent
cargo install --path remotefs-relay
cargo install --path remotefs-nfs
```

### Quick Start

#### 1. Development Setup (Single Machine)

For testing and development, you can run all components on a single machine:

```bash
# Start relay server (terminal 1)
cd examples/relay
CONFIG_FILE=simple_config.toml ./start_relay.sh

# Start agent (terminal 2)
cd examples/agent
CONFIG_FILE=simple_config.toml ./start_agent.sh

# Start NFS server (terminal 3)
cd examples/nfs
remotefs-nfs --config simple_config.toml start

# Mount filesystem (terminal 4)
cd examples/nfs
sudo ./mount_example.sh
```

#### 2. Production Setup (Multiple Machines)

**Relay Server (Cloud/Central Server):**
```bash
# Deploy relay server
cd examples/relay
./deploy.sh production

# Start service
sudo systemctl enable remotefs-relay
sudo systemctl start remotefs-relay
```

**Agent (Remote File Server):**
```bash
# Deploy agent
cd examples/agent
./deploy.sh production

# Configure access paths in /etc/remotefs/agent.toml
# Start service
sudo systemctl enable remotefs-agent
sudo systemctl start remotefs-agent
```

**NFS Client (Local Machine):**
```bash
# Start NFS server
remotefs-nfs --config nfs_config.toml start

# Mount via NFS (Linux)
sudo mount -t nfs -o vers=3,tcp,port=2049 127.0.0.1:/ /mnt/remotefs

# Mount via NFS (macOS) 
sudo mount_nfs -o vers=3,tcp,port=2049 127.0.0.1:/ /mnt/remotefs

# Or use the helper
remotefs-nfs mount mount /mnt/remotefs
```

#### 3. Docker Deployment

```bash
# Complete stack with Docker Compose
cd examples/relay
docker-compose up -d

# Check status
docker-compose ps
docker-compose logs remotefs-relay
```

#### 4. Kubernetes Deployment

```bash
# Generate and deploy manifests
cd examples/relay
./deploy.sh kubernetes
kubectl apply -f k8s/

# Monitor deployment
kubectl get pods -n remotefs
kubectl logs -f deployment/remotefs-relay -n remotefs
```

## Configuration

### Client Configuration (client.toml)

```toml
[client]
client_id = "laptop-001"
relay_url = "wss://relay.example.com:8080/ws"

[[mount_points]]
remote_path = "/home/user/projects"
local_path = "/mnt/remote-projects"
agent_id = "server-001"

[cache]
directory = "~/.cache/remotefs/client"
max_size_gb = 5.0
ttl_seconds = 3600

[security]
key_file = "~/.config/remotefs/client.key"
cert_file = "~/.config/remotefs/client.crt"
```

### Agent Configuration (agent.toml)

```toml
[agent]
agent_id = "server-001"  
relay_url = "wss://relay.example.com:8080/ws"

[access]
allowed_paths = ["/home/user", "/opt/data"]
denied_paths = ["/etc", "/root"]
max_file_size = 10737418240  # 10GB

[security]
key_file = "~/.config/remotefs/agent.key"
cert_file = "~/.config/remotefs/agent.crt"
```

### NFS Server Configuration (nfs.toml)

```toml
# RemoteFS NFS Server Configuration
host = "127.0.0.1"        # NFS server bind address
port = 2049               # NFS server port
agents = [                # RemoteFS agent endpoints
    "ws://127.0.0.1:8080",
    "ws://remote-server:8080"
]
connection_timeout = 30   # Connection timeout in seconds
request_timeout = 60      # Request timeout in seconds
max_connections = 100     # Maximum concurrent connections
debug = false             # Enable debug logging

[auth]
enabled = true
token = "your-secure-token-here"

[performance]
cache_enabled = true
cache_size_mb = 256
read_buffer_size = 65536    # 64KB
write_buffer_size = 65536   # 64KB
compression_enabled = true
```

## NFS Usage Guide

### Starting the NFS Server

```bash
# Generate example configuration
remotefs-nfs config generate

# Edit the configuration file
# Default location: ~/.config/remotefs/nfs.toml (Linux/macOS)
vim ~/.config/remotefs/nfs.toml

# Start the NFS server
remotefs-nfs start

# Or with custom config
remotefs-nfs --config /path/to/nfs.toml start
```

### Mounting Filesystems

#### Linux Mount Commands
```bash
# Create mount point
sudo mkdir -p /mnt/remotefs

# Basic mount
sudo mount -t nfs -o vers=3,tcp,port=2049 127.0.0.1:/ /mnt/remotefs

# Optimized mount (recommended for production)
sudo mount -t nfs -o vers=3,tcp,port=2049,mountport=2049,rsize=1048576,wsize=1048576,async \
  127.0.0.1:/ /mnt/remotefs

# Soft mount with interrupts (recommended for unreliable networks)
sudo mount -t nfs -o vers=3,tcp,port=2049,soft,intr,timeo=60,retrans=3 \
  127.0.0.1:/ /mnt/remotefs
```

#### macOS Mount Commands  
```bash
# Create mount point
sudo mkdir -p /mnt/remotefs

# Basic mount
sudo mount_nfs -o vers=3,tcp,port=2049,mountport=2049 127.0.0.1:/ /mnt/remotefs

# Optimized mount (recommended for production)
sudo mount_nfs -o vers=3,tcp,port=2049,mountport=2049,rsize=1048576,wsize=1048576 \
  127.0.0.1:/ /mnt/remotefs

# Soft mount with interrupts
sudo mount_nfs -o vers=3,tcp,port=2049,soft,intr 127.0.0.1:/ /mnt/remotefs
```

#### Using Mount Helpers
```bash
# Show mount command for current platform
remotefs-nfs mount show /mnt/remotefs

# Automatically mount (detects OS and uses appropriate command)
remotefs-nfs mount mount /mnt/remotefs

# Unmount
remotefs-nfs mount unmount /mnt/remotefs
# Or standard command
sudo umount /mnt/remotefs
```

### Persistent Mounts

#### Linux - /etc/fstab
```bash
# Add to /etc/fstab for automatic mounting at boot
echo "127.0.0.1:/ /mnt/remotefs nfs vers=3,tcp,port=2049,_netdev 0 0" | sudo tee -a /etc/fstab

# Test the fstab entry
sudo mount -a
```

#### macOS - Auto Mount
```bash
# Create auto mount script
sudo mkdir -p /Library/LaunchDaemons
sudo tee /Library/LaunchDaemons/com.remotefs.mount.plist << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.remotefs.mount</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/sbin/mount_nfs</string>
        <string>-o</string>
        <string>vers=3,tcp,port=2049</string>
        <string>127.0.0.1:/</string>
        <string>/mnt/remotefs</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
EOF

# Load the launch daemon
sudo launchctl load /Library/LaunchDaemons/com.remotefs.mount.plist
```

## Use Cases

### Development Environment

Access source code and development tools on a remote server:

```bash
# Mount remote development directory
remotefs-client mount --remote-path /home/dev/workspace --local-path ~/remote-workspace

# Use local IDE with remote files
code ~/remote-workspace/my-project
```

### Dependency Sharing

Share package caches and dependencies:

```bash  
# Mount remote Maven repository
remotefs-client mount --remote-path /home/user/.m2 --local-path ~/.m2 --read-only

# Mount remote npm cache
remotefs-client mount --remote-path /home/user/.npm --local-path ~/.npm --read-only
```

## Security

- All file data is encrypted end-to-end between client and agent
- Key exchange uses X25519 ECDH for perfect forward secrecy
- Sessions use unique encryption keys that rotate periodically
- Agent validates all operations against configured access controls
- Optional mutual TLS authentication for additional security

## Management and Monitoring

### Status Monitoring

```bash
# Check relay server status
cd examples/relay
./relay_utils.sh status
./relay_utils.sh health

# Check agent status
cd examples/agent
./agent_utils.sh status
./agent_utils.sh health

# Check NFS server status
remotefs-nfs status

# Check NFS mount status
cd examples/nfs
./test_mount.sh
```

### Performance Testing

```bash
# Benchmark relay server
./relay_utils.sh benchmark

# Test agent performance
./agent_utils.sh benchmark

# Test NFS mount performance
cd examples/nfs
./test_mount.sh
```

### Metrics and Logging

```bash
# View metrics (Prometheus format)
curl http://localhost:9091/metrics  # Relay
curl http://localhost:8081/metrics  # Agent

# View logs
./relay_utils.sh logs
./agent_utils.sh logs

# Monitor in real-time with Grafana
# Access http://localhost:3000 after Docker Compose deployment
```

### Configuration Management

```bash
# Validate configurations
toml-check examples/relay/relay_config.toml
toml-check examples/agent/agent_config.toml

# Generate configurations from templates
./deploy.sh development  # Creates development configs
./deploy.sh production   # Creates production configs
```

## Performance

- **Local caching** reduces latency for frequently accessed files
- **Compression** reduces network bandwidth usage (LZ4)
- **Parallel operations** support concurrent file access
- **Connection pooling** minimizes connection overhead
- **Load balancing** distributes load across multiple agents
- **Circuit breakers** prevent cascading failures
- **Health monitoring** ensures optimal routing

## Development

### Project Structure

```
remotefs/
├── remotefs-common/    # Shared types and utilities
├── remotefs-client/    # WebSocket client library
├── remotefs-agent/     # Remote file system agent
├── remotefs-relay/     # Cloud relay server
├── remotefs-nfs/       # Cross-platform NFS server
└── examples/
    ├── nfs/            # NFS examples and configurations
    ├── relay/          # Relay server examples  
    └── agent/          # Agent examples
```

### Building

```bash
# Build all components
cargo build --workspace

# Run tests  
cargo test --workspace

# Build release binaries
cargo build --workspace --release
```

### Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality  
5. Submit a pull request

## Troubleshooting

### Common Issues

#### Connection Problems
```bash
# Check if services are running
sudo systemctl status remotefs-relay
sudo systemctl status remotefs-agent

# Check network connectivity
telnet relay.example.com 9090
curl -f http://agent.example.com:8081/health

# Check firewall settings
sudo ufw status
sudo iptables -L
```

#### NFS Mount Issues  
```bash
# Check if NFS server is running
remotefs-nfs status
netstat -ln | grep :2049

# Check mount permissions
ls -la /mnt/remotefs
mountpoint /mnt/remotefs

# Check NFS client availability
# Linux
which mount.nfs
# macOS
which mount_nfs

# Check NFS logs
dmesg | grep nfs
journalctl -u remotefs-nfs

# Force unmount if stuck
sudo umount -f /mnt/remotefs
# Or use helper
remotefs-nfs mount unmount /mnt/remotefs
```

#### Performance Issues
```bash
# Check system resources
top -p $(pgrep remotefs)
iostat -x 1

# Monitor network usage
iftop -i eth0
ss -tuln | grep :9090

# Check cache usage
du -sh ~/.cache/remotefs
```

#### Configuration Problems
```bash
# Validate TOML syntax
toml-check /etc/remotefs/relay.toml

# Check configuration permissions
ls -la /etc/remotefs/

# Test with minimal config
cp examples/relay/simple_config.toml test.toml
./start_relay.sh
```

### Debug Mode

Enable detailed logging for troubleshooting:

```bash
# Environment variable
export RUST_LOG=debug

# Or in configuration
[logging]
level = "debug"
log_requests = true
```

### Getting Help

- Check component-specific README files in each directory
- Review example configurations in `examples/` directories
- Use management scripts for automated diagnostics
- Enable debug logging for detailed error information

#### Component Documentation
- [Client Library](remotefs-client/README.md)
- [Agent Server](remotefs-agent/README.md)  
- [Relay Server](remotefs-relay/README.md)
- [NFS Integration](remotefs-nfs/README.md)
- [Common Library](remotefs-common/README.md)
- [Migration Guide](MIGRATION_GUIDE.md) - FUSE to NFS migration

## License

This project is licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.

## Status

🎉 **RemoteFS is feature-complete and ready for testing and evaluation!**

All core components have been successfully implemented with comprehensive documentation, examples, and deployment tools.

### ✅ Completed Components

#### Core Infrastructure
- **remotefs-common** - Complete protocol definitions, encryption, configuration system, utilities
- **remotefs-agent** - Production-ready agent with security, monitoring, filesystem access, and health checks
- **remotefs-client** - Full-featured WebSocket client library with connection pooling, retries, and load balancing
- **remotefs-relay** - Intelligent relay server with load balancing, service discovery, and high availability
- **remotefs-nfs** - Cross-platform NFS v3 server for transparent filesystem mounting (Linux & macOS)

#### Advanced Features
- **Service Discovery** - Consul integration for dynamic agent discovery
- **Load Balancing** - Multiple strategies (round-robin, weighted, least connections, hash-based)
- **High Availability** - Circuit breaker patterns, health monitoring, automatic failover
- **Security** - End-to-end encryption, TLS/SSL, authentication, access control
- **Monitoring** - Prometheus metrics, comprehensive logging, performance tracking
- **Caching** - Intelligent attribute and data caching for performance optimization

#### Deployment & Operations
- **Configuration Management** - TOML-based with validation and environment variable support
- **Container Support** - Docker and Docker Compose configurations
- **Kubernetes** - Complete manifest generation and deployment scripts
- **System Integration** - systemd services, init scripts, process management
- **Management Tools** - Status monitoring, health checks, performance benchmarking

### 📚 Documentation & Examples

- **Comprehensive READMEs** - Detailed documentation for each component
- **Configuration Examples** - Development, production, and high-availability configurations
- **Deployment Scripts** - Automated deployment for multiple environments
- **Management Utilities** - Operational scripts for monitoring and maintenance
- **Usage Examples** - Real-world scenarios and integration patterns

### 🏗️ Current Capabilities

#### Core Functionality
- Secure WebSocket connections between all components
- End-to-end encryption with ChaCha20-Poly1305 and X25519 key exchange
- Comprehensive access control and path validation
- High-performance file operations with concurrent processing
- Robust error handling and automatic recovery

#### Scalability & Performance
- Support for thousands of concurrent connections
- Intelligent load balancing across multiple agents
- Connection pooling and multiplexing
- Local caching for improved performance
- Compression for bandwidth optimization

#### Enterprise Features
- Service discovery with Consul integration
- Health monitoring and alerting
- Metrics collection and visualization
- Configuration management and validation
- Multi-environment deployment support

### 🚧 Platform Notes

- **Linux**: Full support including NFS mounting with nfs-common package
- **macOS**: Full support including built-in NFS client (no additional packages needed)
- **Windows**: Not currently supported (NFS client available but not tested - contributions welcome)

### 📋 Production Readiness

RemoteFS includes all features necessary for production deployment:

✅ **Security**: End-to-end encryption, access control, TLS support  
✅ **Reliability**: Health checks, circuit breakers, automatic failover  
✅ **Scalability**: Load balancing, connection pooling, service discovery  
✅ **Monitoring**: Metrics, logging, alerting, performance tracking  
✅ **Operations**: Deployment scripts, management tools, documentation  
✅ **Testing**: Unit tests, integration tests, benchmarking tools  

## Roadmap

- [x] Architecture and protocol design
- [x] Common library with encryption and protocols
- [x] Relay server implementation with load balancing and service discovery
- [x] Agent implementation with security and monitoring
- [x] Client library implementation with advanced features
- [x] NFS filesystem integration (cross-platform: Linux & macOS)
- [x] Configuration and deployment tools
- [x] Comprehensive testing and benchmarking
- [x] Documentation and examples
- [x] Production deployment patterns

### Future Enhancements

- [ ] Windows support and native filesystem integration
- [ ] Advanced caching strategies and cache synchronization
- [ ] Distributed consensus for multi-relay deployments
- [ ] Performance optimizations and protocol enhancements
- [ ] GUI applications and management interfaces
- [ ] Integration with cloud storage providers
