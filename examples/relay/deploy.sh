#!/bin/bash

# RemoteFS Relay Server Deployment Script
# This script helps deploy the relay server in various environments

set -e

# Configuration
DEPLOYMENT_TYPE="${1:-development}"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
CONFIG_DIR="${CONFIG_DIR:-/etc/remotefs}"
DATA_DIR="${DATA_DIR:-/var/lib/remotefs}"
LOG_DIR="${LOG_DIR:-/var/log/remotefs}"
SERVICE_USER="${SERVICE_USER:-remotefs}"

function show_usage() {
    echo "Usage: $0 [DEPLOYMENT_TYPE]"
    echo
    echo "Deployment Types:"
    echo "  development  - Development setup (default)"
    echo "  production   - Production deployment with systemd"
    echo "  docker       - Docker container deployment"
    echo "  kubernetes   - Kubernetes deployment (generates manifests)"
    echo
    echo "Environment Variables:"
    echo "  INSTALL_DIR - Installation directory (default: /usr/local/bin)"
    echo "  CONFIG_DIR  - Configuration directory (default: /etc/remotefs)"
    echo "  DATA_DIR    - Data directory (default: /var/lib/remotefs)"
    echo "  LOG_DIR     - Log directory (default: /var/log/remotefs)"
    echo "  SERVICE_USER - Service user (default: remotefs)"
}

function check_requirements() {
    echo "Checking deployment requirements..."
    
    # Check if Rust/Cargo is available for building
    if ! command -v cargo &> /dev/null; then
        echo "ERROR: cargo not found. Please install Rust:"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    
    # Check if we can build the relay
    if [ ! -f "../../remotefs-relay/Cargo.toml" ]; then
        echo "ERROR: remotefs-relay source not found"
        echo "Please run this script from the examples/relay directory"
        exit 1
    fi
    
    echo "✓ Requirements check passed"
}

function build_relay() {
    echo "Building RemoteFS Relay Server..."
    
    cd ../../
    
    echo "Building release binary..."
    cargo build --release --package remotefs-relay
    
    if [ ! -f "target/release/remotefs-relay" ]; then
        echo "ERROR: Build failed - binary not found"
        exit 1
    fi
    
    echo "✓ Build completed successfully"
    cd examples/relay/
}

function deploy_development() {
    echo "Setting up development deployment..."
    
    build_relay
    
    # Create local directories
    mkdir -p ./data ./logs ./config
    
    # Copy binary to local bin (if desired)
    cp ../../target/release/remotefs-relay ./remotefs-relay
    chmod +x ./remotefs-relay
    
    # Copy configuration
    cp simple_config.toml ./config/relay.toml
    
    echo "✓ Development deployment complete!"
    echo
    echo "To start the relay server:"
    echo "  ./remotefs-relay --config ./config/relay.toml"
    echo
    echo "Or use the startup script:"
    echo "  CONFIG_FILE=./config/relay.toml ./start_relay.sh"
}

function create_system_user() {
    echo "Creating system user: $SERVICE_USER"
    
    if id "$SERVICE_USER" &>/dev/null; then
        echo "User $SERVICE_USER already exists"
    else
        sudo useradd --system --shell /bin/false --home-dir "$DATA_DIR" --create-home "$SERVICE_USER"
        echo "✓ Created system user: $SERVICE_USER"
    fi
}

function create_directories() {
    echo "Creating system directories..."
    
    sudo mkdir -p "$INSTALL_DIR" "$CONFIG_DIR" "$DATA_DIR" "$LOG_DIR"
    sudo chown "$SERVICE_USER:$SERVICE_USER" "$DATA_DIR" "$LOG_DIR"
    sudo chmod 755 "$CONFIG_DIR"
    sudo chmod 750 "$DATA_DIR" "$LOG_DIR"
    
    echo "✓ System directories created"
}

function install_binary() {
    echo "Installing relay binary..."
    
    sudo cp ../../target/release/remotefs-relay "$INSTALL_DIR/"
    sudo chmod 755 "$INSTALL_DIR/remotefs-relay"
    sudo chown root:root "$INSTALL_DIR/remotefs-relay"
    
    echo "✓ Binary installed to $INSTALL_DIR/remotefs-relay"
}

function install_config() {
    echo "Installing configuration..."
    
    sudo cp relay_config.toml "$CONFIG_DIR/relay.toml"
    sudo chown root:$SERVICE_USER "$CONFIG_DIR/relay.toml"
    sudo chmod 640 "$CONFIG_DIR/relay.toml"
    
    echo "✓ Configuration installed to $CONFIG_DIR/relay.toml"
}

function install_systemd_service() {
    echo "Installing systemd service..."
    
    cat << EOF | sudo tee /etc/systemd/system/remotefs-relay.service > /dev/null
[Unit]
Description=RemoteFS Relay Server
After=network.target
Wants=network.target

[Service]
Type=simple
ExecStart=$INSTALL_DIR/remotefs-relay --config $CONFIG_DIR/relay.toml
ExecReload=/bin/kill -HUP \$MAINPID
Restart=always
RestartSec=5
User=$SERVICE_USER
Group=$SERVICE_USER
WorkingDirectory=$DATA_DIR

# Security hardening
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ReadWritePaths=$DATA_DIR $LOG_DIR

# Resource limits
LimitNOFILE=65536
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
EOF
    
    sudo systemctl daemon-reload
    echo "✓ Systemd service installed"
}

function deploy_production() {
    echo "Setting up production deployment..."
    
    build_relay
    create_system_user
    create_directories
    install_binary
    install_config
    install_systemd_service
    
    echo "✓ Production deployment complete!"
    echo
    echo "To start the service:"
    echo "  sudo systemctl enable remotefs-relay"
    echo "  sudo systemctl start remotefs-relay"
    echo
    echo "To check status:"
    echo "  sudo systemctl status remotefs-relay"
    echo "  sudo journalctl -u remotefs-relay -f"
}

function generate_dockerfile() {
    echo "Generating Dockerfile..."
    
    cat << 'EOF' > Dockerfile
FROM rust:1.70 as builder

WORKDIR /app
COPY . .
RUN cargo build --release --package remotefs-relay

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/remotefs-relay /usr/local/bin/
COPY examples/relay/relay_config.toml /etc/remotefs/relay.toml

RUN groupadd -r remotefs && useradd -r -g remotefs remotefs
RUN mkdir -p /var/lib/remotefs /var/log/remotefs && \
    chown remotefs:remotefs /var/lib/remotefs /var/log/remotefs

EXPOSE 9090 9091
USER remotefs:remotefs

VOLUME ["/var/lib/remotefs", "/var/log/remotefs", "/etc/remotefs"]

HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:9091/metrics || exit 1

CMD ["remotefs-relay", "--config", "/etc/remotefs/relay.toml"]
EOF
    
    echo "✓ Dockerfile generated"
}

function generate_docker_compose() {
    echo "Generating docker-compose.yml..."
    
    # The docker-compose.yml already exists, so just inform the user
    if [ -f "docker-compose.yml" ]; then
        echo "✓ docker-compose.yml already exists"
    else
        echo "⚠ docker-compose.yml not found - should be generated separately"
    fi
}

function deploy_docker() {
    echo "Setting up Docker deployment..."
    
    generate_dockerfile
    generate_docker_compose
    
    echo "Building Docker image..."
    docker build -t remotefs-relay:latest -f Dockerfile ../../
    
    echo "✓ Docker deployment complete!"
    echo
    echo "To run with Docker:"
    echo "  docker run -p 9090:9090 -p 9091:9091 remotefs-relay:latest"
    echo
    echo "To run with Docker Compose:"
    echo "  docker-compose up -d"
}

function generate_k8s_manifests() {
    echo "Generating Kubernetes manifests..."
    
    mkdir -p k8s
    
    # Namespace
    cat << EOF > k8s/namespace.yaml
apiVersion: v1
kind: Namespace
metadata:
  name: remotefs
  labels:
    name: remotefs
EOF
    
    # ConfigMap
    cat << EOF > k8s/configmap.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: remotefs-relay-config
  namespace: remotefs
data:
  relay.toml: |
$(sed 's/^/    /' relay_config.toml)
EOF
    
    # Deployment
    cat << EOF > k8s/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: remotefs-relay
  namespace: remotefs
  labels:
    app: remotefs-relay
spec:
  replicas: 3
  selector:
    matchLabels:
      app: remotefs-relay
  template:
    metadata:
      labels:
        app: remotefs-relay
    spec:
      containers:
      - name: remotefs-relay
        image: remotefs-relay:latest
        ports:
        - containerPort: 9090
          name: relay
        - containerPort: 9091
          name: metrics
        volumeMounts:
        - name: config-volume
          mountPath: /etc/remotefs
          readOnly: true
        - name: data-volume
          mountPath: /var/lib/remotefs
        - name: logs-volume
          mountPath: /var/log/remotefs
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /metrics
            port: 9091
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /metrics
            port: 9091
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: config-volume
        configMap:
          name: remotefs-relay-config
      - name: data-volume
        persistentVolumeClaim:
          claimName: remotefs-data-pvc
      - name: logs-volume
        emptyDir: {}
EOF
    
    # Service
    cat << EOF > k8s/service.yaml
apiVersion: v1
kind: Service
metadata:
  name: remotefs-relay-service
  namespace: remotefs
  labels:
    app: remotefs-relay
spec:
  selector:
    app: remotefs-relay
  ports:
  - name: relay
    port: 9090
    targetPort: 9090
  - name: metrics
    port: 9091
    targetPort: 9091
  type: ClusterIP
EOF
    
    # Ingress
    cat << EOF > k8s/ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: remotefs-relay-ingress
  namespace: remotefs
  annotations:
    nginx.ingress.kubernetes.io/proxy-read-timeout: "3600"
    nginx.ingress.kubernetes.io/proxy-send-timeout: "3600"
    nginx.ingress.kubernetes.io/websocket-services: "remotefs-relay-service"
spec:
  rules:
  - host: relay.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: remotefs-relay-service
            port:
              number: 9090
EOF
    
    # PVC
    cat << EOF > k8s/pvc.yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: remotefs-data-pvc
  namespace: remotefs
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 10Gi
EOF
    
    echo "✓ Kubernetes manifests generated in k8s/ directory"
}

function deploy_kubernetes() {
    echo "Setting up Kubernetes deployment..."
    
    if ! command -v kubectl &> /dev/null; then
        echo "ERROR: kubectl not found"
        echo "Please install kubectl to deploy to Kubernetes"
        exit 1
    fi
    
    generate_k8s_manifests
    
    echo "✓ Kubernetes deployment manifests ready!"
    echo
    echo "To deploy to Kubernetes:"
    echo "  kubectl apply -f k8s/"
    echo
    echo "To check status:"
    echo "  kubectl get pods -n remotefs"
    echo "  kubectl logs -f deployment/remotefs-relay -n remotefs"
}

function cleanup_deployment() {
    echo "Cleaning up deployment files..."
    
    case $DEPLOYMENT_TYPE in
        development)
            rm -f ./remotefs-relay
            rm -rf ./data ./logs ./config
            ;;
        docker)
            docker rmi remotefs-relay:latest || true
            rm -f Dockerfile
            ;;
        kubernetes)
            kubectl delete -f k8s/ || true
            rm -rf k8s/
            ;;
    esac
    
    echo "✓ Cleanup complete"
}

# Main execution
case $DEPLOYMENT_TYPE in
    development)
        check_requirements
        deploy_development
        ;;
    production)
        check_requirements
        deploy_production
        ;;
    docker)
        check_requirements
        deploy_docker
        ;;
    kubernetes)
        check_requirements
        deploy_kubernetes
        ;;
    cleanup)
        cleanup_deployment
        ;;
    help|--help|-h)
        show_usage
        ;;
    *)
        echo "Unknown deployment type: $DEPLOYMENT_TYPE"
        echo
        show_usage
        exit 1
        ;;
esac
