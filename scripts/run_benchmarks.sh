#!/usr/bin/env bash
# ============================================================================
# LuxTensor Benchmark Runner
# Runs all Criterion benchmarks and outputs a summary table.
# Usage: ./scripts/run_benchmarks.sh [--tps-only | --perf-only | --hnsw-only]
# ============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
LUXTENSOR_DIR="$ROOT_DIR/luxtensor"

# Colors
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

TIMESTAMP=$(date '+%Y-%m-%d_%H%M%S')
RESULTS_DIR="$LUXTENSOR_DIR/benchmark_results"
mkdir -p "$RESULTS_DIR"
SUMMARY_FILE="$RESULTS_DIR/benchmark_summary_${TIMESTAMP}.md"

echo -e "${CYAN}============================================${NC}"
echo -e "${CYAN}  LuxTensor Benchmark Suite${NC}"
echo -e "${CYAN}  $(date)${NC}"
echo -e "${CYAN}============================================${NC}"
echo ""

# Parse arguments
RUN_TPS=true
RUN_PERF=true
RUN_HNSW=true
RUN_STRESS=true

if [[ "${1:-}" == "--tps-only" ]]; then
    RUN_PERF=false; RUN_HNSW=false; RUN_STRESS=false
elif [[ "${1:-}" == "--perf-only" ]]; then
    RUN_TPS=false; RUN_HNSW=false; RUN_STRESS=false
elif [[ "${1:-}" == "--hnsw-only" ]]; then
    RUN_TPS=false; RUN_PERF=false; RUN_STRESS=false
fi

cd "$LUXTENSOR_DIR"

# Write summary header
cat > "$SUMMARY_FILE" << 'EOF'
# LuxTensor Benchmark Results

EOF
echo "**Date:** $(date '+%Y-%m-%d %H:%M:%S')" >> "$SUMMARY_FILE"
echo "**System:** $(uname -s) $(uname -m)" >> "$SUMMARY_FILE"
echo "**Rust:** $(rustc --version 2>/dev/null || echo 'unknown')" >> "$SUMMARY_FILE"
echo "" >> "$SUMMARY_FILE"

# --------------------------------------------------------------------------
# Helper: extract criterion results from target/criterion
# --------------------------------------------------------------------------
extract_criterion_results() {
    local bench_name="$1"
    local label="$2"

    echo "" >> "$SUMMARY_FILE"
    echo "## $label" >> "$SUMMARY_FILE"
    echo "" >> "$SUMMARY_FILE"
    echo "| Benchmark | Time (avg) | Throughput |" >> "$SUMMARY_FILE"
    echo "|-----------|-----------|------------|" >> "$SUMMARY_FILE"

    # Parse estimate files from criterion output
    local base_dir="$LUXTENSOR_DIR/target/criterion"
    if [[ -d "$base_dir" ]]; then
        find "$base_dir" -path "*/new/estimates.json" -type f 2>/dev/null | sort | while read -r est_file; do
            # Extract benchmark name from path
            local rel_path="${est_file#$base_dir/}"
            local bname
            bname=$(echo "$rel_path" | sed 's|/new/estimates.json||' | tr '/' ' › ')

            # Parse mean point estimate (nanoseconds) from JSON
            local mean_ns
            mean_ns=$(grep -o '"point_estimate":[0-9.]*' "$est_file" | head -1 | cut -d: -f2)

            if [[ -n "$mean_ns" ]]; then
                # Convert to human-readable
                local time_str
                if (( $(echo "$mean_ns > 1000000000" | bc -l 2>/dev/null || echo 0) )); then
                    time_str=$(printf "%.2f s" "$(echo "$mean_ns / 1000000000" | bc -l)")
                elif (( $(echo "$mean_ns > 1000000" | bc -l 2>/dev/null || echo 0) )); then
                    time_str=$(printf "%.2f ms" "$(echo "$mean_ns / 1000000" | bc -l)")
                elif (( $(echo "$mean_ns > 1000" | bc -l 2>/dev/null || echo 0) )); then
                    time_str=$(printf "%.2f µs" "$(echo "$mean_ns / 1000" | bc -l)")
                else
                    time_str=$(printf "%.0f ns" "$mean_ns")
                fi

                # Calculate ops/sec
                local ops_sec
                ops_sec=$(printf "%.0f" "$(echo "1000000000 / $mean_ns" | bc -l 2>/dev/null || echo 0)")

                echo "| $bname | $time_str | ${ops_sec} ops/s |" >> "$SUMMARY_FILE"
            fi
        done
    fi
}

# --------------------------------------------------------------------------
# Run benchmarks
# --------------------------------------------------------------------------

EXIT_CODE=0

if $RUN_TPS; then
    echo -e "${GREEN}▶ Running TPS benchmarks...${NC}"
    TPS_LOG="$RESULTS_DIR/tps_benchmark_${TIMESTAMP}.log"
    if cargo bench --bench tps_benchmark -p luxtensor-tests 2>&1 | tee "$TPS_LOG"; then
        echo -e "${GREEN}✓ TPS benchmarks complete${NC}"
        extract_criterion_results "tps_benchmark" "TPS Benchmarks"

        # Also extract [TPS] lines from stderr captured in log
        echo "" >> "$SUMMARY_FILE"
        echo "### Effective TPS Summary" >> "$SUMMARY_FILE"
        echo '```' >> "$SUMMARY_FILE"
        grep '\[TPS\]' "$TPS_LOG" >> "$SUMMARY_FILE" 2>/dev/null || echo "(no TPS lines captured)" >> "$SUMMARY_FILE"
        echo '```' >> "$SUMMARY_FILE"
    else
        echo -e "${YELLOW}⚠ TPS benchmarks failed (see $TPS_LOG)${NC}"
        EXIT_CODE=1
    fi
    echo ""
fi

if $RUN_PERF; then
    echo -e "${GREEN}▶ Running performance benchmarks...${NC}"
    PERF_LOG="$RESULTS_DIR/performance_${TIMESTAMP}.log"
    if cargo bench --bench performance_benchmarks -p luxtensor-tests 2>&1 | tee "$PERF_LOG"; then
        echo -e "${GREEN}✓ Performance benchmarks complete${NC}"
        extract_criterion_results "performance_benchmarks" "Performance Benchmarks"
    else
        echo -e "${YELLOW}⚠ Performance benchmarks failed (see $PERF_LOG)${NC}"
        EXIT_CODE=1
    fi
    echo ""
fi

if $RUN_STRESS; then
    echo -e "${GREEN}▶ Running stress tests...${NC}"
    STRESS_LOG="$RESULTS_DIR/stress_${TIMESTAMP}.log"
    if cargo bench --bench stress_tests -p luxtensor-tests 2>&1 | tee "$STRESS_LOG"; then
        echo -e "${GREEN}✓ Stress tests complete${NC}"
        extract_criterion_results "stress_tests" "Stress Tests"
    else
        echo -e "${YELLOW}⚠ Stress tests failed (see $STRESS_LOG)${NC}"
        EXIT_CODE=1
    fi
    echo ""
fi

if $RUN_HNSW; then
    echo -e "${GREEN}▶ Running HNSW benchmarks...${NC}"
    HNSW_LOG="$RESULTS_DIR/hnsw_${TIMESTAMP}.log"
    # Search for any hnsw bench targets across the workspace
    HNSW_BENCHES=$(cargo bench --bench 'hnsw*' -p luxtensor-tests 2>&1 || true)
    echo "$HNSW_BENCHES" | tee "$HNSW_LOG"

    # Also try luxtensor-core which may contain HNSW benches
    HNSW_CORE=$(cargo bench --bench 'hnsw*' -p luxtensor-core 2>&1 || true)
    echo "$HNSW_CORE" | tee -a "$HNSW_LOG"

    echo -e "${GREEN}✓ HNSW benchmarks attempted${NC}"
    echo ""
fi

# --------------------------------------------------------------------------
# Final summary
# --------------------------------------------------------------------------

echo "" >> "$SUMMARY_FILE"
echo "---" >> "$SUMMARY_FILE"
echo "*Generated by \`scripts/run_benchmarks.sh\` on $(date)*" >> "$SUMMARY_FILE"

echo -e "${CYAN}============================================${NC}"
echo -e "${CYAN}  Benchmark run complete${NC}"
echo -e "${CYAN}  Summary: $SUMMARY_FILE${NC}"
echo -e "${CYAN}============================================${NC}"

# Print the summary to stdout
echo ""
cat "$SUMMARY_FILE"

exit $EXIT_CODE
