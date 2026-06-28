#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Profile-Guided Optimization (PGO) Build Script — Antigravity Engine
# ─────────────────────────────────────────────────────────────────────────────
#
# PGO uses runtime profiling data to guide compiler optimizations, yielding
# 10-20% performance improvements for typical web server workloads.
#
# Prerequisites:
#   - Rust nightly or stable 1.70+ with llvm-tools-preview
#   - The llvm-profdata tool (installed via rustup component add llvm-tools-preview)
#
# Usage:
#   ./scripts/pgo_build.sh
#
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

PROFILE_DIR="$PROJECT_ROOT/target/pgo-profiles"
MERGED_PROFILE="$PROFILE_DIR/merged.profdata"

echo "═══════════════════════════════════════════════════════════════════"
echo "  Antigravity PGO Build Pipeline"
echo "═══════════════════════════════════════════════════════════════════"

# ── Step 0: Ensure llvm-tools-preview is installed ────────────────────────────
echo ""
echo "▸ Step 0: Ensuring llvm-tools-preview component is installed..."
rustup component add llvm-tools-preview 2>/dev/null || true

# ── Step 1: Instrumented Build ────────────────────────────────────────────────
echo ""
echo "▸ Step 1: Building instrumented binary for profile data collection..."
rm -rf "$PROFILE_DIR"
mkdir -p "$PROFILE_DIR"

RUSTFLAGS="-Cprofile-generate=$PROFILE_DIR" \
    cargo build --release --target "$(rustc -vV | grep host | awk '{print $2}')"

BINARY="$PROJECT_ROOT/target/$(rustc -vV | grep host | awk '{print $2}')/release/antigravity"

if [ ! -f "$BINARY" ]; then
    echo "ERROR: Instrumented binary not found at $BINARY"
    exit 1
fi

echo "  ✓ Instrumented binary built: $BINARY"

# ── Step 2: Collect Profile Data ──────────────────────────────────────────────
echo ""
echo "▸ Step 2: Run the instrumented binary under a representative workload."
echo ""
echo "  Start the server:"
echo "    $BINARY"
echo ""
echo "  Then run your load test / benchmark suite against it. For example:"
echo "    wrk -t4 -c50 -d30s http://localhost:8443/health"
echo "    # Also exercise key API paths: /api/v1/auth/challenge, etc."
echo ""
echo "  After the workload finishes, stop the server (Ctrl+C) and re-run"
echo "  this script — it will detect existing profile data and proceed to Step 3."
echo ""

# Check if profile data exists from a previous run
PROFILE_COUNT=$(find "$PROFILE_DIR" -name "*.profraw" 2>/dev/null | wc -l)

if [ "$PROFILE_COUNT" -eq 0 ]; then
    echo "  ⚠  No .profraw files found in $PROFILE_DIR."
    echo "     Run the instrumented binary under load, then re-run this script."
    exit 0
fi

echo "  ✓ Found $PROFILE_COUNT profile data file(s)"

# ── Step 3: Merge Profile Data ────────────────────────────────────────────────
echo ""
echo "▸ Step 3: Merging profile data..."

LLVM_PROFDATA=$(find "$(rustc --print sysroot)" -name "llvm-profdata" -type f | head -1)

if [ -z "$LLVM_PROFDATA" ]; then
    echo "ERROR: llvm-profdata not found. Install with: rustup component add llvm-tools-preview"
    exit 1
fi

"$LLVM_PROFDATA" merge -o "$MERGED_PROFILE" "$PROFILE_DIR"/*.profraw
echo "  ✓ Merged profile written to $MERGED_PROFILE"

# ── Step 4: Optimized Build with PGO Data ─────────────────────────────────────
echo ""
echo "▸ Step 4: Building final optimized binary with PGO data..."

RUSTFLAGS="-Cprofile-use=$MERGED_PROFILE -Cllvm-args=-pgo-warn-missing-function" \
    cargo build --release --target "$(rustc -vV | grep host | awk '{print $2}')"

echo ""
echo "═══════════════════════════════════════════════════════════════════"
echo "  ✓ PGO-optimized binary built successfully!"
echo "  Binary: $BINARY"
echo "═══════════════════════════════════════════════════════════════════"
