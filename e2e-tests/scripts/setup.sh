#!/bin/bash
set -e

# RemoteFS E2E Test Environment Setup Script

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

print_info "RemoteFS E2E Test Environment Setup"
print_info "Project root: $PROJECT_ROOT"

# Check dependencies
check_dependencies() {
    print_info "Checking dependencies..."
    
    if ! command -v docker &> /dev/null; then
        print_error "Docker is not installed or not in PATH"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        print_error "Docker Compose is not installed or not in PATH"
        exit 1
    fi
    
    print_success "Dependencies check passed"
}

# Create required directories
setup_directories() {
    print_info "Setting up directories..."
    
    mkdir -p "$PROJECT_ROOT/test-results"
    mkdir -p "/tmp/remotefs-mount"
    
    # Ensure proper permissions
    chmod 755 "$PROJECT_ROOT/test-results"
    chmod 755 "/tmp/remotefs-mount"
    
    print_success "Directories created"
}

# Build Docker images
build_images() {
    print_info "Building Docker images..."
    
    cd "$PROJECT_ROOT"
    
    # Build all images
    docker-compose build --no-cache
    
    if [ $? -eq 0 ]; then
        print_success "Docker images built successfully"
    else
        print_error "Failed to build Docker images"
        exit 1
    fi
}

# Start services
start_services() {
    print_info "Starting RemoteFS services..."
    
    cd "$PROJECT_ROOT"
    
    # Start services in background
    docker-compose up -d
    
    if [ $? -eq 0 ]; then
        print_success "Services started successfully"
    else
        print_error "Failed to start services"
        exit 1
    fi
}

# Wait for services to be healthy
wait_for_services() {
    print_info "Waiting for services to be ready..."
    
    local max_wait=120  # 2 minutes
    local wait_time=0
    
    while [ $wait_time -lt $max_wait ]; do
        if docker-compose ps | grep -q "Up (healthy)"; then
            print_success "Services are ready"
            return 0
        fi
        
        sleep 5
        wait_time=$((wait_time + 5))
        print_info "Waiting... ($wait_time/$max_wait seconds)"
    done
    
    print_error "Services failed to become ready within timeout"
    print_info "Current service status:"
    docker-compose ps
    
    # Show logs for debugging
    print_info "Service logs:"
    docker-compose logs --tail=20
    
    return 1
}

# Show service status
show_status() {
    print_info "Service status:"
    docker-compose ps
    
    echo ""
    print_info "Available endpoints:"
    echo "  - Relay Server: http://localhost:8080"
    echo "  - Relay WebSocket: ws://localhost:8081" 
    echo "  - NFS Server: localhost:2049"
    echo "  - Test Results: $PROJECT_ROOT/test-results"
    echo "  - Mount Point: /tmp/remotefs-mount"
}

# Main setup function
main() {
    print_info "Starting RemoteFS E2E Test Environment Setup"
    
    check_dependencies
    setup_directories
    build_images
    start_services
    
    if wait_for_services; then
        show_status
        print_success "E2E test environment is ready!"
        print_info "You can now run tests with: ./scripts/run_tests.sh"
    else
        print_error "Setup failed - services are not ready"
        exit 1
    fi
}

# Handle script arguments
case "${1:-}" in
    "help"|"-h"|"--help")
        echo "RemoteFS E2E Test Environment Setup"
        echo ""
        echo "Usage: $0 [options]"
        echo ""
        echo "Options:"
        echo "  help, -h, --help    Show this help message"
        echo "  --no-build          Skip building Docker images"
        echo "  --force-recreate    Force recreate containers"
        echo ""
        echo "This script will:"
        echo "  1. Check dependencies (Docker, Docker Compose)"
        echo "  2. Create required directories"
        echo "  3. Build Docker images for all components"
        echo "  4. Start all services"
        echo "  5. Wait for services to be healthy"
        ;;
    "--no-build")
        print_info "Skipping Docker image build..."
        check_dependencies
        setup_directories
        start_services
        wait_for_services && show_status
        ;;
    "--force-recreate")
        print_info "Force recreating containers..."
        cd "$PROJECT_ROOT"
        docker-compose down -v
        main
        ;;
    "")
        main
        ;;
    *)
        print_error "Unknown option: $1"
        print_info "Use '$0 help' for usage information"
        exit 1
        ;;
esac
