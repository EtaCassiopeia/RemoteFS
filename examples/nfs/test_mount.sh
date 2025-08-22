#!/bin/bash

# RemoteFS NFS Test Script
# Tests NFS mounting and basic file operations

set -e

# Configuration
NFS_HOST="127.0.0.1"
NFS_PORT="2049"
MOUNT_POINT="/tmp/remotefs-test"
TEST_FILE="$MOUNT_POINT/test_file.txt"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if NFS server is accessible
check_nfs_server() {
    print_step "Checking if NFS server is accessible..."
    if timeout 5 bash -c "</dev/tcp/$NFS_HOST/$NFS_PORT"; then
        print_success "NFS server is accessible at $NFS_HOST:$NFS_PORT"
    else
        print_error "Cannot connect to NFS server at $NFS_HOST:$NFS_PORT"
        echo "Make sure the RemoteFS NFS server is running:"
        echo "  cargo run --bin remotefs-nfs"
        exit 1
    fi
}

# Create test mount point
create_test_mount_point() {
    print_step "Creating test mount point: $MOUNT_POINT"
    sudo mkdir -p "$MOUNT_POINT"
    print_success "Mount point created"
}

# Mount NFS
mount_test_nfs() {
    print_step "Mounting NFS filesystem..."
    
    # Detect OS for appropriate mount command
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        sudo mount -t nfs -o vers=3,tcp,port=$NFS_PORT,mountport=$NFS_PORT \
            $NFS_HOST:/ "$MOUNT_POINT"
    elif [[ "$OSTYPE" == "darwin"* ]]; then
        sudo mount_nfs -o vers=3,tcp,port=$NFS_PORT,mountport=$NFS_PORT \
            $NFS_HOST:/ "$MOUNT_POINT"
    else
        print_error "Unsupported OS: $OSTYPE"
        exit 1
    fi
    
    print_success "NFS mounted successfully"
}

# Test basic file operations
test_file_operations() {
    print_step "Testing basic file operations..."
    
    # Test write
    echo "Hello from RemoteFS NFS test!" | sudo tee "$TEST_FILE" > /dev/null
    print_success "Write test passed"
    
    # Test read
    if sudo cat "$TEST_FILE" | grep -q "Hello from RemoteFS"; then
        print_success "Read test passed"
    else
        print_error "Read test failed"
        exit 1
    fi
    
    # Test directory listing
    if sudo ls "$MOUNT_POINT" > /dev/null; then
        print_success "Directory listing test passed"
    else
        print_error "Directory listing test failed"
        exit 1
    fi
    
    # Test file removal
    sudo rm "$TEST_FILE"
    if [[ ! -f "$TEST_FILE" ]]; then
        print_success "File removal test passed"
    else
        print_error "File removal test failed"
        exit 1
    fi
}

# Test mount status
test_mount_status() {
    print_step "Checking mount status..."
    if mount | grep -q "$MOUNT_POINT"; then
        print_success "Mount is active and visible in mount table"
        mount | grep "$MOUNT_POINT"
    else
        print_error "Mount not found in mount table"
        exit 1
    fi
}

# Cleanup
cleanup() {
    print_step "Cleaning up..."
    
    # Unmount
    if mount | grep -q "$MOUNT_POINT"; then
        sudo umount "$MOUNT_POINT" || sudo umount -f "$MOUNT_POINT"
        print_success "Unmounted NFS filesystem"
    fi
    
    # Remove test mount point
    if [[ -d "$MOUNT_POINT" ]]; then
        sudo rmdir "$MOUNT_POINT"
        print_success "Removed test mount point"
    fi
}

# Main test execution
main() {
    echo "RemoteFS NFS Test Suite"
    echo "======================"
    echo ""
    
    # Set up cleanup trap
    trap cleanup EXIT
    
    check_nfs_server
    create_test_mount_point
    mount_test_nfs
    test_mount_status
    test_file_operations
    
    echo ""
    print_success "All tests passed! RemoteFS NFS is working correctly."
    echo ""
}

# Check for sudo
if [[ $EUID -ne 0 ]]; then
    echo "This test script requires sudo privileges for mounting"
    echo "Usage: sudo ./test_mount.sh"
    exit 1
fi

# Run tests
main "$@"
