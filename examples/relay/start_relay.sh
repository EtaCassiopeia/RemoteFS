#!/bin/bash

# RemoteFS Relay Server Startup Script
# This script helps start the relay server with proper validation and monitoring

set -e

# Configuration
CONFIG_FILE="${CONFIG_FILE:-simple_config.toml}"
LOG_LEVEL="${LOG_LEVEL:-info}"
BIND_PORT="${BIND_PORT:-9090}"
METRICS_PORT="${METRICS_PORT:-9091}"
DAEMON_MODE="${DAEMON_MODE:-false}"

echo "=== RemoteFS Relay Server Startup ==="
echo "Config file: $CONFIG_FILE"
echo "Log level: $LOG_LEVEL"
echo "Bind port: $BIND_PORT"
echo "Metrics port: $METRICS_PORT"
echo "Daemon mode: $DAEMON_MODE"
echo

# Check if configuration file exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo "ERROR: Configuration file not found: $CONFIG_FILE"
    echo "Available example configurations:"
    echo "  - simple_config.toml (development)"
    echo "  - relay_config.toml (production)"
    echo "  - ha_config.toml (high availability)"
    exit 1
fi

# Validate configuration syntax
echo "Validating configuration..."
if ! toml get "$CONFIG_FILE" server.server_id >/dev/null 2>&1; then
    echo "ERROR: Invalid TOML configuration file"
    exit 1
fi
echo "✓ Configuration syntax is valid"

# Check if relay binary exists
if ! command -v remotefs-relay &> /dev/null; then
    echo "ERROR: remotefs-relay binary not found"
    echo "Please build the relay server:"
    echo "  cargo build --release --package remotefs-relay"
    echo "  # Then add target/release to your PATH"
    exit 1
fi

# Check port availability
echo "Checking port availability..."
if netstat -an | grep -q ":$BIND_PORT "; then
    echo "WARNING: Port $BIND_PORT appears to be in use"
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

if netstat -an | grep -q ":$METRICS_PORT "; then
    echo "WARNING: Metrics port $METRICS_PORT appears to be in use"
fi

# Create log directory if needed
LOG_FILE=$(toml get "$CONFIG_FILE" logging.file_path 2>/dev/null | tr -d '"' || echo "")
if [ -n "$LOG_FILE" ] && [ "$LOG_FILE" != "null" ]; then
    LOG_DIR=$(dirname "$LOG_FILE")
    if [ ! -d "$LOG_DIR" ]; then
        echo "Creating log directory: $LOG_DIR"
        sudo mkdir -p "$LOG_DIR"
        sudo chown "$USER:$USER" "$LOG_DIR"
    fi
fi

# Set up signal handlers for graceful shutdown
cleanup() {
    echo
    echo "Shutting down relay server..."
    if [ -n "$RELAY_PID" ]; then
        kill -TERM "$RELAY_PID" 2>/dev/null || true
        wait "$RELAY_PID" 2>/dev/null || true
    fi
    echo "Relay server stopped."
}
trap cleanup SIGINT SIGTERM

# Export environment variables for config substitution
export RELAY_AUTH_SECRET="${RELAY_AUTH_SECRET:-dev-secret-key}"
export REDIS_PASSWORD="${REDIS_PASSWORD:-}"

echo "Starting RemoteFS Relay Server..."
echo "Press Ctrl+C to stop"
echo

# Start the relay server
if [ "$DAEMON_MODE" = "true" ]; then
    echo "Starting in daemon mode..."
    nohup remotefs-relay \
        --config "$CONFIG_FILE" \
        --log-level "$LOG_LEVEL" \
        > /dev/null 2>&1 &
    RELAY_PID=$!
    echo "Relay server started with PID: $RELAY_PID"
    echo "Check logs for status: tail -f $LOG_FILE"
else
    remotefs-relay \
        --config "$CONFIG_FILE" \
        --log-level "$LOG_LEVEL" &
    RELAY_PID=$!
    
    # Wait for the process
    wait "$RELAY_PID"
fi
