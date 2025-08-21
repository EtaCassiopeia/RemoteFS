# RemoteFS - Secure Remote File System Access

RemoteFS is an encrypted remote file system mounting solution that allows you to securely access files from a remote machine through a cloud relay server. It provides FUSE filesystem mounts for transparent local access to remote directories.

## Architecture

RemoteFS consists of three main components:

- **Client**: Runs on your local machine, provides FUSE mounts
- **Agent**: Runs on the remote machine, provides file system access
- **Relay Server**: Cloud-based intermediary for secure communication

```
[Local Client] <--encrypted--> [Cloud Relay] <--encrypted--> [Remote Agent]
      |                                                           |
   FUSE Mount                                               File System
```

## Features

- **End-to-end encryption** using ChaCha20-Poly1305
- **Secure key exchange** using X25519 ECDH
- **FUSE filesystem** for transparent local access
- **Local caching** for improved performance
- **Access control** on the remote agent
- **Connection resilience** with automatic reconnection
- **Cross-platform** support (Linux, macOS)

## Quick Start

### 1. Install RemoteFS

```bash
# Build from source
cargo build --release

# Install binaries
cargo install --path remotefs-client
cargo install --path remotefs-agent  
cargo install --path remotefs-relay
```

### 2. Start the Relay Server (Cloud)

```bash
# Generate default configuration
remotefs-relay --init-config

# Edit relay configuration
vim ~/.config/remotefs/relay.toml

# Start relay server
remotefs-relay --config ~/.config/remotefs/relay.toml
```

### 3. Start the Agent (Remote Machine)

```bash
# Generate default configuration  
remotefs-agent --init-config

# Edit agent configuration
vim ~/.config/remotefs/agent.toml

# Start agent
remotefs-agent --config ~/.config/remotefs/agent.toml
```

### 4. Mount Remote Directory (Local Machine)

```bash
# Generate default configuration
remotefs-client --init-config

# Edit client configuration
vim ~/.config/remotefs/client.toml

# Add mount point and start client
remotefs-client mount --remote-path /home/user/projects --local-path /mnt/remote
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

## Performance

- Local caching reduces latency for frequently accessed files
- Compression reduces network bandwidth usage
- Parallel operations support concurrent file access
- Connection pooling minimizes connection overhead

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

## License

This project is licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.

## Status

🚧 **This project is currently under active development and not ready for production use.**

The core architecture has been designed and the foundational components are being implemented. See the [Architecture Document](ARCHITECTURE.md) for detailed design information.

## Roadmap

- [x] Architecture and protocol design
- [x] Common library with encryption and protocols
- [ ] Relay server implementation
- [ ] Agent implementation  
- [ ] FUSE client implementation
- [ ] Configuration and deployment tools
- [ ] Testing and benchmarking
- [ ] Documentation and examples
- [ ] Production hardening
