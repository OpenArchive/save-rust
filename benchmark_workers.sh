#!/bin/bash
# Benchmark script to test server performance with different worker counts

set -e

SERVER_URL="http://127.0.0.1:8080"
BASE_DIR="/tmp/save-benchmark-$$"
REQUESTS=50
CONCURRENT=5

cleanup() {
    echo "Cleaning up..."
    pkill -f "save-server" || true
    sleep 1
    rm -rf "$BASE_DIR" || true
}
trap cleanup EXIT

# Function to benchmark with a given worker count
benchmark_workers() {
    local workers=$1
    echo ""
    echo "=========================================="
    echo "Testing with $workers worker(s)"
    echo "=========================================="
    
    # Start server in background
    mkdir -p "$BASE_DIR"
    SAVE_WORKER_COUNT=$workers cargo run --bin save-server -- "$BASE_DIR" > /tmp/server-$$.log 2>&1 &
    SERVER_PID=$!
    
    # Wait for server to be ready
    echo "Waiting for server to start..."
    for i in {1..30}; do
        if curl -s "$SERVER_URL/health" > /dev/null 2>&1; then
            echo "Server is ready!"
            break
        fi
        sleep 1
    done
    
    if ! curl -s "$SERVER_URL/health" > /dev/null 2>&1; then
        echo "ERROR: Server failed to start (check /tmp/server-$$.log)"
        kill $SERVER_PID 2>/dev/null || true
        return 1
    fi
    
    # Run benchmark using parallel curl requests
    echo "Running benchmark: $REQUESTS requests with $CONCURRENT concurrent connections..."
    start_time=$(date +%s.%N)
    
    # Create a function to make requests
    make_request() {
        curl -s -w "%{time_total}\n" -o /dev/null "$SERVER_URL/health"
    }
    
    # Export function and run parallel requests
    export -f make_request
    export SERVER_URL
    
    # Run requests in parallel batches
    times=$(seq 1 $REQUESTS | xargs -P $CONCURRENT -I {} bash -c 'make_request')
    
    end_time=$(date +%s.%N)
    total_time=$(echo "$end_time - $start_time" | bc)
    
    # Calculate average response time
    avg_time=$(echo "$times" | awk '{sum+=$1; count++} END {if(count>0) print sum/count; else print 0}')
    req_per_sec=$(echo "scale=2; $REQUESTS / $total_time" | bc)
    
    echo "Total time: ${total_time}s"
    echo "Average response time: ${avg_time}s"
    echo "Requests per second: $req_per_sec"
    
    # Stop server
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
    sleep 2
}

# Check if required tools are available
if ! command -v curl &> /dev/null; then
    echo "ERROR: curl is required but not installed"
    exit 1
fi

if ! command -v bc &> /dev/null; then
    echo "ERROR: bc is required but not installed"
    exit 1
fi

echo "Benchmarking server with different worker counts"
echo "Requests: $REQUESTS, Concurrent: $CONCURRENT"
echo ""

# Test with different worker counts
benchmark_workers 1
sleep 2
benchmark_workers 2
sleep 2
benchmark_workers 4

echo ""
echo "Benchmark complete!"
