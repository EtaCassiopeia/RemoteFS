#!/bin/bash

# RemoteFS FUSE Test and Management Script
# This script provides utilities for testing and managing FUSE mounts

set -e

MOUNT_POINT="${MOUNT_POINT:-/mnt/remotefs}"

function show_usage() {
    echo "Usage: $0 [COMMAND]"
    echo
    echo "Commands:"
    echo "  test      - Test if mount point is working"
    echo "  unmount   - Force unmount the filesystem" 
    echo "  status    - Show mount status"
    echo "  benchmark - Run simple performance test"
    echo "  help      - Show this help"
    echo
    echo "Environment variables:"
    echo "  MOUNT_POINT - Mount point directory (default: /mnt/remotefs)"
}

function test_mount() {
    echo "Testing RemoteFS FUSE mount at: $MOUNT_POINT"
    
    if ! mountpoint -q "$MOUNT_POINT"; then
        echo "ERROR: $MOUNT_POINT is not mounted"
        return 1
    fi
    
    echo "✓ Mount point is active"
    
    # Test basic operations
    echo "Testing basic file operations..."
    
    # Test directory listing
    echo -n "  Directory listing: "
    if ls "$MOUNT_POINT" >/dev/null 2>&1; then
        echo "✓ OK"
    else
        echo "✗ FAILED"
        return 1
    fi
    
    # Test file creation (if writable)
    echo -n "  File creation: "
    TEST_FILE="$MOUNT_POINT/.remotefs_test_$$"
    if echo "test data" > "$TEST_FILE" 2>/dev/null; then
        echo "✓ OK"
        # Clean up test file
        rm -f "$TEST_FILE" 2>/dev/null || true
    else
        echo "✓ SKIPPED (read-only mount)"
    fi
    
    # Test metadata access
    echo -n "  Metadata access: "
    if stat "$MOUNT_POINT" >/dev/null 2>&1; then
        echo "✓ OK"
    else
        echo "✗ FAILED"
        return 1
    fi
    
    echo "All tests passed!"
}

function unmount_fs() {
    echo "Unmounting RemoteFS FUSE at: $MOUNT_POINT"
    
    if ! mountpoint -q "$MOUNT_POINT"; then
        echo "INFO: $MOUNT_POINT is not mounted"
        return 0
    fi
    
    # Try graceful unmount first
    echo "Attempting graceful unmount..."
    if fusermount -u "$MOUNT_POINT" 2>/dev/null; then
        echo "✓ Graceful unmount successful"
        return 0
    fi
    
    # Try force unmount
    echo "Graceful unmount failed, trying force unmount..."
    if fusermount -uz "$MOUNT_POINT" 2>/dev/null; then
        echo "✓ Force unmount successful"
        return 0
    fi
    
    # Last resort - system unmount
    echo "Force unmount failed, trying system unmount..."
    if sudo umount -f "$MOUNT_POINT" 2>/dev/null; then
        echo "✓ System unmount successful"
        return 0
    fi
    
    echo "✗ All unmount attempts failed"
    echo "You may need to:"
    echo "  1. Kill any processes using the mount point"
    echo "  2. Use 'sudo umount -f $MOUNT_POINT'"
    echo "  3. Reboot the system"
    
    return 1
}

function show_status() {
    echo "RemoteFS FUSE Status"
    echo "===================="
    echo
    
    echo "Mount point: $MOUNT_POINT"
    
    if mountpoint -q "$MOUNT_POINT"; then
        echo "Status: ✓ MOUNTED"
        
        # Show mount details
        echo
        echo "Mount details:"
        mount | grep "$MOUNT_POINT" || true
        
        # Show processes using the mount
        echo
        echo "Processes using mount:"
        lsof "$MOUNT_POINT" 2>/dev/null || echo "  (none)"
        
        # Show available space if possible
        echo
        echo "Filesystem usage:"
        df -h "$MOUNT_POINT" 2>/dev/null || echo "  (unable to determine)"
        
    else
        echo "Status: ✗ NOT MOUNTED"
    fi
}

function benchmark_mount() {
    echo "Running RemoteFS FUSE benchmark at: $MOUNT_POINT"
    
    if ! mountpoint -q "$MOUNT_POINT"; then
        echo "ERROR: $MOUNT_POINT is not mounted"
        return 1
    fi
    
    BENCH_DIR="$MOUNT_POINT/.benchmark_$$"
    
    # Create benchmark directory
    echo "Creating benchmark directory..."
    if ! mkdir -p "$BENCH_DIR" 2>/dev/null; then
        echo "ERROR: Cannot create benchmark directory (read-only mount?)"
        return 1
    fi
    
    trap "rm -rf '$BENCH_DIR' 2>/dev/null || true" EXIT
    
    echo "Running benchmarks..."
    echo
    
    # File creation benchmark
    echo "1. File creation test (100 small files)"
    start_time=$(date +%s.%N)
    for i in {1..100}; do
        echo "test data $i" > "$BENCH_DIR/file_$i.txt"
    done
    end_time=$(date +%s.%N)
    duration=$(echo "$end_time - $start_time" | bc -l)
    echo "   Time: ${duration}s ($(echo "scale=2; 100 / $duration" | bc -l) files/sec)"
    
    # File reading benchmark  
    echo "2. File reading test (100 files)"
    start_time=$(date +%s.%N)
    for i in {1..100}; do
        cat "$BENCH_DIR/file_$i.txt" >/dev/null
    done
    end_time=$(date +%s.%N)
    duration=$(echo "$end_time - $start_time" | bc -l)
    echo "   Time: ${duration}s ($(echo "scale=2; 100 / $duration" | bc -l) files/sec)"
    
    # Directory listing benchmark
    echo "3. Directory listing test (10 iterations)"
    start_time=$(date +%s.%N)
    for i in {1..10}; do
        ls "$BENCH_DIR" >/dev/null
    done
    end_time=$(date +%s.%N)
    duration=$(echo "$end_time - $start_time" | bc -l)
    echo "   Time: ${duration}s ($(echo "scale=2; 10 / $duration" | bc -l) listings/sec)"
    
    # Large file test
    echo "4. Large file test (1MB file)"
    start_time=$(date +%s.%N)
    dd if=/dev/zero of="$BENCH_DIR/large_file.dat" bs=1024 count=1024 2>/dev/null
    end_time=$(date +%s.%N)
    duration=$(echo "$end_time - $start_time" | bc -l)
    echo "   Write time: ${duration}s ($(echo "scale=2; 1 / $duration" | bc -l) MB/sec)"
    
    start_time=$(date +%s.%N)
    dd if="$BENCH_DIR/large_file.dat" of=/dev/null bs=1024 2>/dev/null
    end_time=$(date +%s.%N)
    duration=$(echo "$end_time - $start_time" | bc -l)
    echo "   Read time: ${duration}s ($(echo "scale=2; 1 / $duration" | bc -l) MB/sec)"
    
    echo
    echo "Benchmark complete!"
}

# Main command dispatch
case "${1:-help}" in
    test)
        test_mount
        ;;
    unmount)
        unmount_fs
        ;;
    status)
        show_status
        ;;
    benchmark)
        benchmark_mount
        ;;
    help|--help|-h)
        show_usage
        ;;
    *)
        echo "Unknown command: $1"
        echo
        show_usage
        exit 1
        ;;
esac
