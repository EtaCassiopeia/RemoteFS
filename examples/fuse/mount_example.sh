#!/bin/bash

# RemoteFS FUSE Mount Example Script
# This script demonstrates how to mount and use RemoteFS with FUSE

set -e

# Configuration
CONFIG_FILE="${CONFIG_FILE:-simple_config.toml}"
MOUNT_POINT="${MOUNT_POINT:-/mnt/remotefs}"
LOG_LEVEL="${LOG_LEVEL:-info}"

echo "=== RemoteFS FUSE Mount Example ==="
echo "Config file: $CONFIG_FILE"
echo "Mount point: $MOUNT_POINT"
echo "Log level: $LOG_LEVEL"
echo

# Check if running as root or with proper permissions
if [ "$EUID" -ne 0 ] && ! groups | grep -q fuse; then
    echo "WARNING: You may need root privileges or membership in the 'fuse' group"
    echo "To add yourself to the fuse group:"
    echo "  sudo usermod -a -G fuse \$USER"
    echo "  # Then logout and login again"
    echo
fi

# Check if mount point exists
if [ ! -d "$MOUNT_POINT" ]; then
    echo "Creating mount point directory: $MOUNT_POINT"
    sudo mkdir -p "$MOUNT_POINT"
    sudo chown "$USER:$USER" "$MOUNT_POINT"
fi

# Check if FUSE is available
if ! command -v fusermount &> /dev/null; then
    echo "ERROR: fusermount not found. Please install FUSE:"
    echo "  Ubuntu/Debian: sudo apt-get install fuse3"
    echo "  CentOS/RHEL: sudo yum install fuse3"
    echo "  Fedora: sudo dnf install fuse3"
    exit 1
fi

# Check if RemoteFS mount binary exists
if ! command -v remotefs-mount &> /dev/null; then
    echo "ERROR: remotefs-mount not found. Please build and install it:"
    echo "  cargo build --release --package remotefs-fuse"
    echo "  # Then add target/release to your PATH"
    exit 1
fi

echo "Starting RemoteFS FUSE mount..."
echo "Press Ctrl+C to stop"
echo

# Start the mount (this will block)
remotefs-mount \
    --config "$CONFIG_FILE" \
    --log-level "$LOG_LEVEL"

echo "Mount stopped."
