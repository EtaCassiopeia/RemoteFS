#!/bin/bash

# RemoteFS NFS Mount Example
# Cross-platform mounting script for Linux and macOS

set -e

# Configuration
NFS_HOST="127.0.0.1"
NFS_PORT="2049"
MOUNT_POINT="/mnt/remotefs"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if script is run as root for mounting
check_sudo() {
    if [[ $EUID -ne 0 ]]; then
        print_error "This script needs to be run with sudo for mounting operations"
        echo "Usage: sudo ./mount_example.sh"
        exit 1
    fi
}

# Detect OS
detect_os() {
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        OS="linux"
        print_info "Detected Linux"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        OS="macos"
        print_info "Detected macOS"
    else
        print_error "Unsupported OS: $OSTYPE"
        exit 1
    fi
}

# Check if NFS client is available
check_nfs_client() {
    if [[ "$OS" == "linux" ]]; then
        if ! command -v mount.nfs &> /dev/null; then
            print_error "NFS client not found. Please install nfs-utils:"
            echo "  Ubuntu/Debian: sudo apt-get install nfs-common"
            echo "  CentOS/RHEL:   sudo yum install nfs-utils"
            echo "  Fedora:        sudo dnf install nfs-utils"
            exit 1
        fi
    elif [[ "$OS" == "macos" ]]; then
        if ! command -v mount_nfs &> /dev/null; then
            print_error "NFS client not available on this macOS version"
            exit 1
        fi
    fi
    print_info "NFS client is available"
}

# Create mount point
create_mount_point() {
    print_info "Creating mount point: $MOUNT_POINT"
    mkdir -p "$MOUNT_POINT"
}

# Mount NFS filesystem
mount_nfs() {
    print_info "Mounting RemoteFS via NFS..."
    
    if [[ "$OS" == "linux" ]]; then
        # Linux mount options
        mount -t nfs -o vers=3,tcp,port=$NFS_PORT,mountport=$NFS_PORT,rsize=1048576,wsize=1048576,async \
            $NFS_HOST:/ "$MOUNT_POINT"
    elif [[ "$OS" == "macos" ]]; then
        # macOS mount options
        mount_nfs -o vers=3,tcp,port=$NFS_PORT,mountport=$NFS_PORT,rsize=1048576,wsize=1048576 \
            $NFS_HOST:/ "$MOUNT_POINT"
    fi
    
    print_info "Successfully mounted RemoteFS at $MOUNT_POINT"
}

# Verify mount
verify_mount() {
    if mount | grep -q "$MOUNT_POINT"; then
        print_info "Mount verified successfully"
        print_info "You can now access your remote files at: $MOUNT_POINT"
    else
        print_error "Mount verification failed"
        exit 1
    fi
}

# Show usage information
show_usage() {
    echo "RemoteFS NFS Mount Example"
    echo "========================="
    echo ""
    echo "This script mounts a RemoteFS NFS server at $MOUNT_POINT"
    echo ""
    echo "Prerequisites:"
    echo "1. RemoteFS NFS server is running on $NFS_HOST:$NFS_PORT"
    echo "2. NFS client tools are installed"
    echo "3. Run this script with sudo privileges"
    echo ""
    echo "To unmount:"
    echo "  sudo umount $MOUNT_POINT"
    echo ""
    echo "To check mount status:"
    echo "  mount | grep remotefs"
    echo ""
}

# Main execution
main() {
    show_usage
    check_sudo
    detect_os
    check_nfs_client
    create_mount_point
    mount_nfs
    verify_mount
    
    echo ""
    print_info "RemoteFS mounted successfully!"
    print_info "Access your files at: $MOUNT_POINT"
    echo ""
    print_warning "To unmount, run: sudo umount $MOUNT_POINT"
}

# Run main function
main "$@"
