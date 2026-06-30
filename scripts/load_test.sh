#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Antigravity Engine — Load Testing Automation Script
# Benchmarks target endpoints (e.g. /health, /auth/challenge) under concurrency.
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

TARGET_URL=${1:-"http://localhost:8443"}
CONCURRENCY=${2:-"50"}
THREADS=${3:-"4"}
DURATION=${4:-"10s"}

echo "═══════════════════════════════════════════════════════════════════"
echo "  Antigravity Load Test Toolkit"
echo "  Target URL:   $TARGET_URL"
echo "  Concurrency:  $CONCURRENCY connections"
echo "  Threads:      $THREADS threads"
echo "  Duration:     $DURATION"
echo "═══════════════════════════════════════════════════════════════════"

# Check if wrk is installed
if ! command -v wrk &> /dev/null; then
    echo "WARNING: 'wrk' load testing tool is not installed."
    echo "To run high-performance benchmarks, install it via your package manager:"
    echo "  - Debian/Ubuntu: sudo apt-get install wrk"
    echo "  - Arch Linux:    sudo pacman -S wrk"
    echo "  - macOS:         brew install wrk"
    echo ""
    echo "Falling back to parallel curl-based check..."
    
    # Simple lightweight parallel test using curl
    start_time=$(date +%s.%N)
    success=0
    failures=0
    
    for i in $(seq 1 "$CONCURRENCY"); do
        (
            status_code=$(curl -s -o /dev/null -w "%{http_code}" "$TARGET_URL/health" || echo "000")
            if [ "$status_code" -eq 200 ]; then
                exit 0
            else
                exit 1
            fi
        ) &
    done
    
    # Wait for all background tasks to finish
    wait
    
    end_time=$(date +%s.%N)
    duration=$(echo "$end_time - $start_time" | bc -l)
    echo "✓ Curl fallback test complete."
    echo "Total Requests: $CONCURRENCY"
    echo "Elapsed Time:   $(printf "%.3f" "$duration") seconds"
    exit 0
fi

# Run wrk load testing
echo ""
echo "▸ Testing GET /health endpoint..."
wrk -t"$THREADS" -c"$CONCURRENCY" -d"$DURATION" --latency "$TARGET_URL/health"

echo ""
echo "▸ Testing GET /api/v1/auth/challenge endpoint..."
wrk -t"$THREADS" -c"$CONCURRENCY" -d"$DURATION" --latency "$TARGET_URL/api/v1/auth/challenge"

echo ""
echo "✓ Load tests completed successfully."
