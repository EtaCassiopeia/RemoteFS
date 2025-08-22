#!/bin/bash

# RemoteFS E2E Test Environment Teardown Script

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

print_info "RemoteFS E2E Test Environment Teardown"
print_info "Project root: $PROJECT_ROOT"

# Unmount filesystems
cleanup_mounts() {
    print_info "Cleaning up filesystem mounts..."
    
    # Unmount FUSE mount if it exists
    if mountpoint -q "/tmp/remotefs-mount" 2>/dev/null; then
        print_info "Unmounting FUSE filesystem..."
        fusermount -u "/tmp/remotefs-mount" 2>/dev/null || umount "/tmp/remotefs-mount" 2>/dev/null || true
    fi
    
    # Clean up NFS test mounts
    for mount_point in /tmp/remotefs-nfs-test /tmp/remotefs-mount; do
        if mountpoint -q "$mount_point" 2>/dev/null; then
            print_info "Unmounting NFS filesystem at $mount_point..."
            sudo umount "$mount_point" 2>/dev/null || true
        fi
        
        # Remove mount point directory if empty
        if [ -d "$mount_point" ]; then
            rmdir "$mount_point" 2>/dev/null || true
        fi
    done
    
    print_success "Filesystem mounts cleaned up"
}

# Stop and remove Docker containers
stop_containers() {
    print_info "Stopping Docker containers..."
    
    cd "$PROJECT_ROOT"
    
    # Stop all services
    docker-compose down
    
    if [ $? -eq 0 ]; then
        print_success "Containers stopped successfully"
    else
        print_warning "Some issues occurred while stopping containers"
    fi
}

# Remove Docker volumes (optional)
cleanup_volumes() {
    print_info "Cleaning up Docker volumes..."
    
    cd "$PROJECT_ROOT"
    
    # Remove volumes
    docker-compose down -v
    
    print_success "Docker volumes cleaned up"
}

# Remove Docker images (optional)
cleanup_images() {
    print_info "Removing Docker images..."
    
    # Get image names from docker-compose
    local images=$(docker-compose images -q 2>/dev/null)
    
    if [ -n "$images" ]; then
        echo "$images" | xargs -r docker rmi -f
        print_success "Docker images removed"
    else
        print_info "No images to remove"
    fi
}

# Clean up test results and logs
cleanup_test_data() {
    local preserve_results=${1:-false}
    
    if [ "$preserve_results" = true ]; then
        print_info "Preserving test results..."
        return 0
    fi
    
    print_info "Cleaning up test data..."
    
    # Clean up test results
    if [ -d "$PROJECT_ROOT/test-results" ]; then
        rm -rf "$PROJECT_ROOT/test-results"/*
        print_success "Test results cleaned up"
    fi
    
    # Clean up temporary files
    rm -f /tmp/remotefs-* 2>/dev/null || true
}

# Show cleanup summary
show_summary() {
    print_info "=== Cleanup Summary ==="
    
    # Check Docker containers
    cd "$PROJECT_ROOT"
    local running_containers=$(docker-compose ps --services --filter "status=running" 2>/dev/null | wc -l)
    
    if [ "$running_containers" -eq 0 ]; then
        print_success "All containers stopped"
    else
        print_warning "Some containers may still be running"
    fi
    
    # Check mounts
    local active_mounts=$(mount | grep -c remotefs 2>/dev/null || echo 0)
    if [ "$active_mounts" -eq 0 ]; then
        print_success "All RemoteFS mounts cleaned up"
    else
        print_warning "Some RemoteFS mounts may still be active"
    fi
    
    print_success "Teardown completed"
}

# Main teardown function
main() {
    local cleanup_everything=false
    local preserve_results=false
    local remove_images=false
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --full)
                cleanup_everything=true
                remove_images=true
                shift
                ;;
            --preserve-results)
                preserve_results=true
                shift
                ;;
            --remove-images)
                remove_images=true
                shift
                ;;
            --help|-h)
                show_usage
                exit 0
                ;;
            *)
                print_error "Unknown option: $1"
                show_usage
                exit 1
                ;;
        esac
    done
    
    print_info "Starting E2E test environment teardown"
    
    # Always do basic cleanup
    cleanup_mounts
    stop_containers
    
    # Additional cleanup based on options
    if [ "$cleanup_everything" = true ]; then
        cleanup_volumes
        cleanup_images
        cleanup_test_data "$preserve_results"
    else
        if [ "$remove_images" = true ]; then
            cleanup_images
        fi
        if [ "$preserve_results" = false ]; then
            cleanup_test_data false
        fi
    fi
    
    show_summary
}

# Show usage information
show_usage() {
    echo "RemoteFS E2E Test Environment Teardown"
    echo ""
    echo "Usage: $0 [options]"
    echo ""
    echo "Options:"
    echo "  --full               Complete cleanup including volumes and images"
    echo "  --preserve-results   Keep test results and logs"
    echo "  --remove-images      Remove Docker images"
    echo "  --help, -h          Show this help message"
    echo ""
    echo "This script will:"
    echo "  1. Unmount any RemoteFS filesystems"
    echo "  2. Stop and remove Docker containers"
    echo "  3. Optionally clean up volumes, images, and test data"
    echo ""
    echo "Examples:"
    echo "  $0                        # Basic cleanup"
    echo "  $0 --full                 # Complete cleanup"
    echo "  $0 --preserve-results     # Cleanup but keep test results"
}

# Handle help argument at top level
case "${1:-}" in
    "help"|"-h"|"--help")
        show_usage
        exit 0
        ;;
esac

# Run main function
main "$@"
