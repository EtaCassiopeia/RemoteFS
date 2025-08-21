# RemoteFS - Distributed Secure Remote File System

RemoteFS is a comprehensive, production-ready distributed file system that enables secure remote file access through intelligent relay servers. It provides multiple client interfaces including FUSE filesystem mounts, WebSocket clients, and programmatic APIs for transparent access to remote directories with end-to-end encryption.

## Architecture

RemoteFS consists of four main components working together to provide seamless, secure file access:

- **Client Library**: Core WebSocket client with load balancing and connection pooling
- **FUSE Integration**: Transparent filesystem mounting for local access
- **Agent**: Secure file system server running on remote machines
- **Relay Server**: Intelligent load balancer and message router with service discovery

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   FUSE Mount    │    │  RemoteFS       │    │  RemoteFS       │    │  RemoteFS       │
│   /mnt/remote   │◄──►│   Client        │◄──►│   Relay         │◄──►│   Agent         │
└─────────────────┘    │   Library       │    │   Server        │    │   Server        │
                       └─────────────────┘    └─────────────────┘    └─────────────────┘
                                │                        │                        │
                                ▼                        ▼                        ▼
                       ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
                       │ Load Balancing  │    │ Service         │    │ Local File      │
                       │ & Connection    │    │ Discovery       │    │ System Access   │
                       │ Pooling         │    │ & Health Checks │    │ with Security   │
                       └─────────────────┘    └─────────────────┘    └─────────────────┘
```

## Features

- **End-to-end encryption** using ChaCha20-Poly1305
- **Secure key exchange** using X25519 ECDH
- **FUSE filesystem** for transparent local access
- **Local caching** for improved performance
- **Access control** on the remote agent
- **Connection resilience** with automatic reconnection
- **Cross-platform** support (Linux, macOS)

## Installation

### Prerequisites

- **Rust 1.70+** for building from source
- **Linux or macOS** (Windows support planned)
- **FUSE3** for filesystem mounting (Linux only)

#### Install FUSE (Linux)
```bash
# Ubuntu/Debian
sudo apt-get install fuse3 libfuse3-dev

# CentOS/RHEL/Fedora
sudo dnf install fuse3 fuse3-devel

# Arch Linux
sudo pacman -S fuse3
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
cargo install --path remotefs-fuse
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

# Start client/FUSE mount (terminal 3)
cd examples/client
CONFIG_FILE=simple_config.toml ./start_client.sh
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

**Client (Local Machine):**
```bash
# FUSE mounting
cd examples/fuse
CONFIG_FILE=fuse_config.toml ./mount_example.sh

# Or programmatic access
cd examples/client
CONFIG_FILE=client_config.toml ./start_client.sh
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

# Check FUSE mount status
cd examples/fuse
./test_mount.sh status
```

### Performance Testing

```bash
# Benchmark relay server
./relay_utils.sh benchmark

# Test agent performance
./agent_utils.sh benchmark

# Test FUSE mount performance
./test_mount.sh benchmark
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
├── remotefs-client/    # FUSE client implementation  
├── remotefs-agent/     # Remote file system agent
├── remotefs-relay/     # Cloud relay server
└── docs/               # Documentation
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

#### FUSE Mount Issues
```bash
# Check FUSE availability
lsmod | grep fuse

# Check mount permissions
ls -la /mnt/remotefs
mountpoint /mnt/remotefs

# Check FUSE logs
dmesg | grep fuse
journalctl -u remotefs-fuse

# Force unmount if stuck
fusermount -uz /mnt/remotefs
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
- [FUSE Integration](remotefs-fuse/README.md)
- [Common Library](remotefs-common/README.md)

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
- **remotefs-fuse** - FUSE filesystem integration for transparent local mounting (Linux support)

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

- **Linux**: Full support including FUSE mounting
- **macOS**: Client and relay supported; FUSE has compatibility limitations
- **Windows**: Not currently supported (contributions welcome)

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
- [x] FUSE filesystem integration (Linux)
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
