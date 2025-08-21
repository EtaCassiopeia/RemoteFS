#!/bin/bash

# RemoteFS Relay Server Management and Testing Utilities
# This script provides various utilities for managing and testing the relay server

set -e

RELAY_HOST="${RELAY_HOST:-127.0.0.1}"
RELAY_PORT="${RELAY_PORT:-9090}"
METRICS_PORT="${METRICS_PORT:-9091}"

function show_usage() {
    echo "Usage: $0 [COMMAND]"
    echo
    echo "Commands:"
    echo "  status      - Check relay server status"
    echo "  health      - Perform health check"
    echo "  metrics     - Display server metrics"
    echo "  agents      - List connected agents"
    echo "  connections - Show active connections"
    echo "  test        - Run connection test"
    echo "  benchmark   - Run performance benchmark"
    echo "  stop        - Stop running relay server"
    echo "  logs        - Tail relay logs"
    echo "  help        - Show this help"
    echo
    echo "Environment variables:"
    echo "  RELAY_HOST - Relay server host (default: 127.0.0.1)"
    echo "  RELAY_PORT - Relay server port (default: 9090)"
    echo "  METRICS_PORT - Metrics server port (default: 9091)"
}

function check_relay_status() {
    echo "Checking RemoteFS Relay Server Status"
    echo "====================================="
    echo
    echo "Host: $RELAY_HOST:$RELAY_PORT"
    echo "Metrics: $RELAY_HOST:$METRICS_PORT"
    echo
    
    # Check if ports are listening
    echo "Port Status:"
    if netstat -an | grep -q ":$RELAY_PORT.*LISTEN"; then
        echo "  ✓ Main port $RELAY_PORT: LISTENING"
    else
        echo "  ✗ Main port $RELAY_PORT: NOT LISTENING"
        return 1
    fi
    
    if netstat -an | grep -q ":$METRICS_PORT.*LISTEN"; then
        echo "  ✓ Metrics port $METRICS_PORT: LISTENING"
    else
        echo "  ✗ Metrics port $METRICS_PORT: NOT LISTENING"
    fi
    
    # Check process
    echo
    echo "Process Status:"
    RELAY_PID=$(pgrep -f "remotefs-relay" || echo "")
    if [ -n "$RELAY_PID" ]; then
        echo "  ✓ Process running (PID: $RELAY_PID)"
        echo "  Memory usage: $(ps -p $RELAY_PID -o rss= | awk '{print int($1/1024)"MB"}')"
        echo "  CPU usage: $(ps -p $RELAY_PID -o %cpu= | awk '{print $1"%"}')"
    else
        echo "  ✗ No relay process found"
        return 1
    fi
}

function perform_health_check() {
    echo "Performing Relay Server Health Check"
    echo "===================================="
    echo
    
    # Basic connectivity test
    echo "1. Testing basic connectivity..."
    if timeout 5 nc -z "$RELAY_HOST" "$RELAY_PORT" 2>/dev/null; then
        echo "   ✓ TCP connection successful"
    else
        echo "   ✗ TCP connection failed"
        return 1
    fi
    
    # WebSocket connection test
    echo "2. Testing WebSocket connection..."
    if command -v websocat &> /dev/null; then
        if timeout 3 echo '{"type":"ping"}' | websocat "ws://$RELAY_HOST:$RELAY_PORT" >/dev/null 2>&1; then
            echo "   ✓ WebSocket connection successful"
        else
            echo "   ✗ WebSocket connection failed"
        fi
    else
        echo "   ⚠ websocat not available, skipping WebSocket test"
    fi
    
    # Metrics endpoint test
    echo "3. Testing metrics endpoint..."
    if curl -s "http://$RELAY_HOST:$METRICS_PORT/metrics" >/dev/null; then
        echo "   ✓ Metrics endpoint accessible"
    else
        echo "   ✗ Metrics endpoint failed"
    fi
    
    echo
    echo "Health check completed"
}

function display_metrics() {
    echo "RemoteFS Relay Server Metrics"
    echo "============================="
    echo
    
    if ! command -v curl &> /dev/null; then
        echo "ERROR: curl is required for metrics display"
        return 1
    fi
    
    METRICS_URL="http://$RELAY_HOST:$METRICS_PORT/metrics"
    
    if ! curl -s "$METRICS_URL" >/dev/null; then
        echo "ERROR: Could not connect to metrics endpoint at $METRICS_URL"
        return 1
    fi
    
    echo "Raw metrics from $METRICS_URL:"
    echo
    curl -s "$METRICS_URL"
}

function list_agents() {
    echo "Connected Agents"
    echo "================"
    echo
    
    # This would typically query the relay's API for agent status
    # For now, show a placeholder
    echo "Agent listing requires relay server API endpoint"
    echo "Check relay logs or metrics for agent connection status"
}

function show_connections() {
    echo "Active Connections"
    echo "=================="
    echo
    
    # Show network connections to relay ports
    echo "Connections to port $RELAY_PORT:"
    netstat -an | grep ":$RELAY_PORT" | head -20
    
    echo
    echo "Connections to metrics port $METRICS_PORT:"
    netstat -an | grep ":$METRICS_PORT" | head -10
}

function run_connection_test() {
    echo "Running Connection Test"
    echo "======================"
    echo
    
    if ! command -v websocat &> /dev/null; then
        echo "ERROR: websocat is required for connection testing"
        echo "Install with: cargo install websocat"
        return 1
    fi
    
    echo "Testing WebSocket connection to $RELAY_HOST:$RELAY_PORT"
    
    # Simple ping test
    echo "Sending ping message..."
    RESPONSE=$(timeout 5 echo '{"type":"ping","data":"test"}' | websocat "ws://$RELAY_HOST:$RELAY_PORT" 2>/dev/null || echo "TIMEOUT")
    
    if [ "$RESPONSE" = "TIMEOUT" ]; then
        echo "✗ Connection test failed (timeout)"
        return 1
    elif [ -n "$RESPONSE" ]; then
        echo "✓ Connection test successful"
        echo "Response: $RESPONSE"
    else
        echo "⚠ Connection established but no response received"
    fi
}

function run_benchmark() {
    echo "Running Relay Server Benchmark"
    echo "==============================="
    echo
    
    if ! command -v websocat &> /dev/null; then
        echo "ERROR: websocat is required for benchmarking"
        return 1
    fi
    
    echo "Running concurrent connection test..."
    
    # Create temporary files for test results
    TEMP_DIR=$(mktemp -d)
    trap "rm -rf $TEMP_DIR" EXIT
    
    CONNECTIONS=10
    MESSAGES=100
    
    echo "Testing $CONNECTIONS concurrent connections with $MESSAGES messages each"
    
    # Launch concurrent connections
    for i in $(seq 1 $CONNECTIONS); do
        {
            start_time=$(date +%s.%N)
            success_count=0
            
            for j in $(seq 1 $MESSAGES); do
                if echo '{"type":"test","id":'$j'}' | timeout 1 websocat "ws://$RELAY_HOST:$RELAY_PORT" >/dev/null 2>&1; then
                    ((success_count++))
                fi
            done
            
            end_time=$(date +%s.%N)
            duration=$(echo "$end_time - $start_time" | bc -l)
            
            echo "$i $success_count $duration" >> "$TEMP_DIR/results"
        } &
    done
    
    # Wait for all background jobs
    wait
    
    # Calculate results
    if [ -f "$TEMP_DIR/results" ]; then
        echo
        echo "Benchmark Results:"
        echo "=================="
        
        total_success=0
        total_duration=0
        connection_count=0
        
        while read -r conn_id success_count duration; do
            total_success=$((total_success + success_count))
            total_duration=$(echo "$total_duration + $duration" | bc -l)
            connection_count=$((connection_count + 1))
            echo "Connection $conn_id: $success_count/$MESSAGES messages (${duration}s)"
        done < "$TEMP_DIR/results"
        
        if [ $connection_count -gt 0 ]; then
            avg_duration=$(echo "scale=3; $total_duration / $connection_count" | bc -l)
            total_messages=$((CONNECTIONS * MESSAGES))
            success_rate=$(echo "scale=2; $total_success * 100 / $total_messages" | bc -l)
            
            echo
            echo "Summary:"
            echo "  Total messages: $total_messages"
            echo "  Successful: $total_success"
            echo "  Success rate: ${success_rate}%"
            echo "  Average duration: ${avg_duration}s"
            
            if [ $(echo "$success_rate > 90" | bc -l) -eq 1 ]; then
                echo "  ✓ Benchmark PASSED"
            else
                echo "  ✗ Benchmark FAILED (success rate too low)"
            fi
        fi
    else
        echo "ERROR: No benchmark results generated"
        return 1
    fi
}

function stop_relay() {
    echo "Stopping RemoteFS Relay Server"
    echo "==============================="
    echo
    
    RELAY_PID=$(pgrep -f "remotefs-relay" || echo "")
    
    if [ -z "$RELAY_PID" ]; then
        echo "No relay server process found"
        return 0
    fi
    
    echo "Found relay process (PID: $RELAY_PID)"
    echo "Sending SIGTERM..."
    
    kill -TERM "$RELAY_PID"
    
    # Wait for graceful shutdown
    for i in {1..10}; do
        if ! kill -0 "$RELAY_PID" 2>/dev/null; then
            echo "✓ Relay server stopped gracefully"
            return 0
        fi
        sleep 1
        echo -n "."
    done
    
    echo
    echo "Graceful shutdown timed out, forcing termination..."
    kill -KILL "$RELAY_PID" 2>/dev/null || true
    echo "✓ Relay server force stopped"
}

function tail_logs() {
    echo "Tailing RemoteFS Relay Logs"
    echo "============================"
    echo
    
    # Common log locations
    LOG_LOCATIONS=(
        "/var/log/remotefs/relay.log"
        "/tmp/remotefs-relay.log"
        "./relay.log"
    )
    
    for log_file in "${LOG_LOCATIONS[@]}"; do
        if [ -f "$log_file" ]; then
            echo "Following log file: $log_file"
            echo "Press Ctrl+C to stop"
            echo
            tail -f "$log_file"
            return 0
        fi
    done
    
    echo "No log file found. Tried:"
    for log_file in "${LOG_LOCATIONS[@]}"; do
        echo "  $log_file"
    done
    echo
    echo "Check relay configuration for log file location"
    return 1
}

# Main command dispatch
case "${1:-help}" in
    status)
        check_relay_status
        ;;
    health)
        perform_health_check
        ;;
    metrics)
        display_metrics
        ;;
    agents)
        list_agents
        ;;
    connections)
        show_connections
        ;;
    test)
        run_connection_test
        ;;
    benchmark)
        run_benchmark
        ;;
    stop)
        stop_relay
        ;;
    logs)
        tail_logs
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
