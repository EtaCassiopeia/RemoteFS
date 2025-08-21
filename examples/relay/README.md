# RemoteFS Relay Server Examples

This directory contains comprehensive examples, configurations, and deployment scripts for the RemoteFS Relay Server.

## Files Overview

### Configuration Examples

1. **`simple_config.toml`** - Minimal configuration for development
   - Single agent setup
   - No authentication or TLS
   - Memory-only storage
   - Perfect for local testing

2. **`relay_config.toml`** - Full production configuration
   - Multiple agents with different weights and regions
   - TLS/SSL support
   - Authentication and rate limiting  
   - Redis storage for session persistence
   - Comprehensive monitoring and logging

3. **`ha_config.toml`** - High-availability configuration
   - Consul service discovery
   - Advanced circuit breaker patterns
   - Production-ready security settings
   - Environment variable configuration

### Management Scripts

4. **`start_relay.sh`** - Relay server startup script
   - Configuration validation
   - Port availability checking
   - Environment setup
   - Graceful shutdown handling

5. **`relay_utils.sh`** - Comprehensive management utilities
   - Status checking and health monitoring
   - Performance benchmarking
   - Connection testing
   - Metrics collection
   - Log analysis

6. **`deploy.sh`** - Multi-environment deployment script
   - Development setup
   - Production systemd deployment  
   - Docker containerization
   - Kubernetes manifests generation

### Container Orchestration

7. **`docker-compose.yml`** - Complete containerized stack
   - RemoteFS Relay Server
   - Redis for session storage
   - Consul for service discovery
   - Sample agents
   - Prometheus + Grafana monitoring
   - Nginx load balancer

## Quick Start

### Development Setup
```bash
# Simple development startup
CONFIG_FILE=simple_config.toml ./start_relay.sh

# Check status
./relay_utils.sh status

# Run health check
./relay_utils.sh health
```

### Production Deployment
```bash
# Deploy to production with systemd
./deploy.sh production

# Start the service
sudo systemctl enable remotefs-relay
sudo systemctl start remotefs-relay
```

### Docker Deployment
```bash
# Deploy as Docker containers
./deploy.sh docker

# Or use Docker Compose for full stack
docker-compose up -d
```

### Kubernetes Deployment
```bash
# Generate Kubernetes manifests
./deploy.sh kubernetes

# Deploy to cluster
kubectl apply -f k8s/
```

## Configuration Guide

### Basic Development Config
- Use `simple_config.toml` for local development
- Single agent at `127.0.0.1:8080`
- No security features enabled
- Memory-only session storage

### Production Config
- Use `relay_config.toml` or `ha_config.toml`
- Enable TLS with proper certificates
- Configure authentication with secure secrets
- Use Redis for session persistence
- Set up proper logging and monitoring

### Environment Variables
```bash
export RELAY_AUTH_SECRET="your-production-secret"
export REDIS_PASSWORD="your-redis-password"
export GRAFANA_PASSWORD="your-grafana-password"
```

## Management and Monitoring

### Status Checking
```bash
./relay_utils.sh status     # Server status
./relay_utils.sh health     # Health check
./relay_utils.sh agents     # Agent list
./relay_utils.sh connections # Active connections
```

### Performance Testing
```bash
./relay_utils.sh test       # Connection test
./relay_utils.sh benchmark  # Performance benchmark
```

### Metrics and Logging
```bash
./relay_utils.sh metrics    # View metrics
./relay_utils.sh logs       # Tail logs
```

## Architecture Patterns

### Single Relay (Development)
```
Client ←→ Relay ←→ Agent
```

### Load Balanced (Production)
```
           ┌─ Relay-1 ─┐
Client ←→ LB ┼─ Relay-2 ─┼ ←→ Agents
           └─ Relay-3 ─┘
```

### Service Discovery (Enterprise)
```
Client ←→ Relay ←→ Consul ←→ Agents
              ↓
           Redis/Storage
```

## Security Configurations

### Development (No Security)
```toml
[authentication]
enabled = false

[server.tls]
enabled = false
```

### Production (Full Security)
```toml
[authentication]
enabled = true
method = "token"

[server.tls]
enabled = true
cert_file = "/etc/ssl/certs/relay.crt"
key_file = "/etc/ssl/private/relay.key"
min_version = "1.3"
```

## Troubleshooting

### Common Issues

1. **Port conflicts**: Check with `netstat -an | grep :9090`
2. **Permission issues**: Ensure proper file permissions for certs/keys
3. **Agent connectivity**: Verify agent addresses and network connectivity
4. **Memory issues**: Monitor with `./relay_utils.sh status`

### Debug Mode
```bash
RUST_LOG=debug ./start_relay.sh
```

### Log Analysis
```bash
./relay_utils.sh logs
grep ERROR /var/log/remotefs/relay.log
```

## Performance Tuning

### Connection Limits
```toml
[server]
max_connections = 5000
connection_timeout = 600
```

### Caching Strategy
```toml
[storage]
type = "redis"  # For session persistence

[storage.redis]
max_connections = 20
idle_timeout = 600
```

### Load Balancing
```toml
[routing]
default_strategy = "weighted"  # Best for production
health_check_interval = 15     # Faster detection
```

## Integration Examples

### With Consul Service Discovery
```bash
# Agents register with Consul
consul services register agent-service.json

# Relay discovers agents automatically
[agents.consul]
service_name = "remotefs-agent"
```

### With Prometheus Monitoring
```yaml
# Prometheus scrape config
- job_name: 'remotefs-relay'
  static_configs:
    - targets: ['relay:9091']
```

### With Nginx Load Balancing
```nginx
upstream remotefs_relay {
    server relay1.example.com:9090;
    server relay2.example.com:9090;
}
```

## Best Practices

### Development
- Use `simple_config.toml` for local testing
- Enable debug logging with `RUST_LOG=debug`
- Use the utility scripts for quick testing

### Staging
- Use `relay_config.toml` with reduced limits
- Test with real agents and clients
- Validate performance with benchmarks

### Production
- Use `ha_config.toml` with proper secrets
- Deploy behind a load balancer
- Monitor with Prometheus/Grafana
- Set up proper alerting

### Security
- Always use TLS in production
- Implement proper authentication
- Use environment variables for secrets
- Regular certificate rotation

## Support and Documentation

- **Main Documentation**: See `../remotefs-relay/README.md`
- **Configuration Reference**: All TOML options documented in example configs
- **API Reference**: WebSocket protocol and HTTP endpoints
- **Troubleshooting Guide**: Common issues and solutions

For more detailed information, refer to the main RemoteFS Relay Server documentation in the `remotefs-relay` directory.
