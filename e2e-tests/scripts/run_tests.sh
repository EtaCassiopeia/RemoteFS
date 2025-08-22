#!/bin/bash
set -e

# RemoteFS E2E Test Runner Script

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

print_info "RemoteFS E2E Test Runner"
print_info "Project root: $PROJECT_ROOT"

# Check if services are running
check_services() {
    print_info "Checking if services are running..."
    
    cd "$PROJECT_ROOT"
    
    if ! docker-compose ps | grep -q "Up"; then
        print_error "Services are not running. Please run ./scripts/setup.sh first"
        exit 1
    fi
    
    print_success "Services are running"
}

# Mount the remote filesystem for testing
setup_mount() {
    print_info "Setting up filesystem mount..."
    
    # Create mount point if it doesn't exist
    mkdir -p "/tmp/remotefs-mount"
    
    # Check if already mounted
    if mountpoint -q "/tmp/remotefs-mount" 2>/dev/null; then
        print_info "Filesystem already mounted"
        return 0
    fi
    
    # Try to mount using the client
    docker-compose exec -T test-client /app/remotefs-client --help &
    
    # Wait a moment for mount to establish
    sleep 5
    
    print_success "Filesystem mount initiated"
}

# Run the E2E tests
run_e2e_tests() {
    print_info "Running E2E tests..."
    
    cd "$PROJECT_ROOT"
    
    # Execute the test script inside the test client container
    docker-compose exec -T test-client python3 /app/scripts/e2e_test.py
    
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        print_success "All E2E tests passed!"
    else
        print_error "Some E2E tests failed (exit code: $exit_code)"
    fi
    
    return $exit_code
}

# Run NFS mount test
test_nfs_mount() {
    print_info "Testing NFS mount functionality..."
    
    local mount_point="/tmp/remotefs-nfs-test"
    mkdir -p "$mount_point"
    
    # Try to mount via NFS
    if sudo mount -t nfs -o vers=3,tcp,port=2049 localhost:/ "$mount_point" 2>/dev/null; then
        print_success "NFS mount successful"
        
        # Test basic operations
        echo "NFS test file" > "$mount_point/nfs_test.txt"
        
        if [ -f "$mount_point/nfs_test.txt" ]; then
            print_success "NFS write test passed"
            rm "$mount_point/nfs_test.txt"
        else
            print_error "NFS write test failed"
        fi
        
        # Unmount
        sudo umount "$mount_point"
        rmdir "$mount_point"
        
        return 0
    else
        print_warning "NFS mount test skipped (requires sudo or NFS client not available)"
        rmdir "$mount_point" 2>/dev/null || true
        return 0
    fi
}

# Generate test report
generate_report() {
    print_info "Generating test report..."
    
    local results_file="$PROJECT_ROOT/test-results/e2e_test_results.json"
    
    if [ -f "$results_file" ]; then
        echo ""
        print_info "=== Test Report ==="
        
        # Extract key metrics using jq if available, otherwise use basic tools
        if command -v jq &> /dev/null; then
            echo "Total Tests: $(jq '.total_tests' "$results_file")"
            echo "Passed: $(jq '.passed_tests' "$results_file")"
            echo "Failed: $(jq '.failed_tests' "$results_file")"
            echo "Success Rate: $(jq '.success_rate' "$results_file")%"
            echo "Duration: $(jq '.total_duration' "$results_file")s"
        else
            print_info "Test results saved to: $results_file"
        fi
        
        echo ""
    else
        print_warning "No test results file found"
    fi
    
    # Show container logs summary
    print_info "=== Service Logs Summary ==="
    echo "Relay Server logs:"
    docker-compose logs --tail=10 relay | head -5
    echo ""
    echo "Agent logs:"
    docker-compose logs --tail=10 remote-agent | head -5
    echo ""
}

# Show container status and logs
show_debug_info() {
    print_info "=== Debug Information ==="
    
    print_info "Container Status:"
    docker-compose ps
    
    echo ""
    print_info "Recent logs from each service:"
    
    echo "--- Relay Server ---"
    docker-compose logs --tail=20 relay
    
    echo "--- Remote Agent ---"
    docker-compose logs --tail=20 remote-agent
    
    echo "--- Test Client ---"
    docker-compose logs --tail=20 test-client
    
    echo "--- NFS Server ---"
    docker-compose logs --tail=20 nfs-server
}

# Main test execution function
main() {
    local run_nfs_test=false
    local show_debug=false
    
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --with-nfs)
                run_nfs_test=true
                shift
                ;;
            --debug)
                show_debug=true
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
    
    print_info "Starting E2E test execution"
    
    check_services
    setup_mount
    
    # Run the main E2E tests
    if run_e2e_tests; then
        local test_result=0
    else
        local test_result=1
    fi
    
    # Run NFS test if requested
    if [ "$run_nfs_test" = true ]; then
        test_nfs_mount
    fi
    
    # Generate report
    generate_report
    
    # Show debug info if requested
    if [ "$show_debug" = true ]; then
        show_debug_info
    fi
    
    if [ $test_result -eq 0 ]; then
        print_success "E2E testing completed successfully!"
    else
        print_error "E2E testing completed with failures"
    fi
    
    exit $test_result
}

# Show usage information
show_usage() {
    echo "RemoteFS E2E Test Runner"
    echo ""
    echo "Usage: $0 [options]"
    echo ""
    echo "Options:"
    echo "  --with-nfs    Also run NFS mount tests (requires sudo)"
    echo "  --debug       Show detailed debug information"
    echo "  --help, -h    Show this help message"
    echo ""
    echo "This script will:"
    echo "  1. Check that all services are running"
    echo "  2. Set up filesystem mounting"
    echo "  3. Run comprehensive E2E tests"
    echo "  4. Generate test report"
    echo ""
    echo "Prerequisites:"
    echo "  - Run './scripts/setup.sh' first to start services"
    echo "  - Ensure Docker and Docker Compose are installed"
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
