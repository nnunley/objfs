#!/bin/bash
# Simulate CI/CD build environment with objfs
# Demonstrates cache sharing between multiple "CI jobs"

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  objfs CI/CD Simulation${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Configuration (simulate CI environment variables)
export OBJFS_REMOTE_ENDPOINT="${OBJFS_REMOTE_ENDPOINT:-http://scheduler-host:50051}"
export OBJFS_REMOTE_INSTANCE="${OBJFS_REMOTE_INSTANCE:-ci-demo}"
export OBJFS_NO_AUTO_WORKER=1
export OBJFS_MIN_REMOTE_SIZE=1

echo -e "${GREEN}Configuration:${NC}"
echo "  OBJFS_REMOTE_ENDPOINT: $OBJFS_REMOTE_ENDPOINT"
echo "  OBJFS_REMOTE_INSTANCE: $OBJFS_REMOTE_INSTANCE"
echo "  OBJFS_NO_AUTO_WORKER:  $OBJFS_NO_AUTO_WORKER"
echo ""

# Check if objfs is installed
if ! command -v cargo-objfs-rustc &> /dev/null; then
    echo -e "${RED}ERROR: cargo-objfs-rustc not found${NC}"
    echo "Please install objfs first:"
    echo "  cd objfs && cargo build --release"
    echo "  sudo cp target/release/cargo-objfs-rustc /usr/local/bin/"
    exit 1
fi

# Check if we have a test project
if [ ! -d "your-project" ]; then
    echo -e "${YELLOW}WARNING: Test project not found${NC}"
    echo "This script expects your-project to exist"
    exit 1
fi

TEST_PROJECT="${1:-your-project}"
TEST_PROJECT=$(eval echo "$TEST_PROJECT")  # Expand ~

echo -e "${GREEN}Test project: ${NC}$TEST_PROJECT"
echo ""

# Function to simulate a CI job
simulate_ci_job() {
    local job_name=$1
    local job_number=$2
    local target=$3

    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}  CI Job: $job_name (#$job_number)${NC}"
    echo -e "${BLUE}========================================${NC}"

    # Create isolated build directory (like CI runner workspace)
    local workspace="/tmp/objfs-ci-demo-job-$job_number"
    rm -rf "$workspace"
    mkdir -p "$workspace"

    echo -e "${YELLOW}Copying project to CI workspace...${NC}"
    cp -r "$TEST_PROJECT" "$workspace/project"
    cd "$workspace/project"

    # Configure cargo to use objfs (like CI setup step)
    echo -e "${YELLOW}Configuring cargo to use objfs...${NC}"
    mkdir -p .cargo
    cat > .cargo/config.toml << 'EOF'
[build]
rustc-wrapper = "/usr/local/bin/cargo-objfs-rustc"
EOF

    # Clean to ensure fresh build (like CI environment)
    echo -e "${YELLOW}Cleaning build artifacts...${NC}"
    cargo clean 2>&1 | head -5

    # Build
    echo -e "${GREEN}Building $job_name...${NC}"
    echo "Command: cargo build --release${target:+ --target $target}"
    echo ""

    local start_time=$(date +%s)

    # Run build and capture objfs statistics
    if [ -n "$target" ]; then
        cargo build --release --target "$target" 2>&1 | \
            grep -E "(Compiling|Finished|objfs)" | \
            tee "/tmp/objfs-ci-job-$job_number.log"
    else
        cargo build --release 2>&1 | \
            grep -E "(Compiling|Finished|objfs)" | \
            tee "/tmp/objfs-ci-job-$job_number.log"
    fi

    local end_time=$(date +%s)
    local duration=$((end_time - start_time))

    echo ""
    echo -e "${GREEN}Job completed in ${duration}s${NC}"

    # Calculate cache statistics
    local total_compilations=$(grep -c "Compiling" "/tmp/objfs-ci-job-$job_number.log" || echo "0")
    local cache_hits=$(grep -c "cache hit" "/tmp/objfs-ci-job-$job_number.log" || echo "0")
    local remote_exec=$(grep -c "remote execution:" "/tmp/objfs-ci-job-$job_number.log" || echo "0")

    echo -e "${BLUE}Statistics:${NC}"
    echo "  Total compilations: $total_compilations"
    echo "  Cache hits:        $cache_hits"
    echo "  Remote executions: $remote_exec"

    if [ "$total_compilations" -gt 0 ]; then
        local hit_rate=$((cache_hits * 100 / total_compilations))
        echo "  Cache hit rate:    ${hit_rate}%"
    fi

    echo ""

    # Return to original directory
    cd - > /dev/null

    # Clean up workspace
    rm -rf "$workspace"
}

# Simulate multiple CI jobs (like parallel PR builds)
echo -e "${BLUE}Simulating 3 parallel CI jobs...${NC}"
echo -e "${YELLOW}(In real CI, these would run on separate runners)${NC}"
echo ""
sleep 2

# Job 1: PR #123 - linux-x86_64 (cold cache)
simulate_ci_job "PR #123 - linux x86_64" 1 "x86_64-unknown-linux-gnu"

# Job 2: PR #124 - linux-x86_64 (should have cache hits from Job 1)
sleep 2
simulate_ci_job "PR #124 - linux x86_64" 2 "x86_64-unknown-linux-gnu"

# Job 3: main branch - linux-x86_64 (should have even more cache hits)
sleep 2
simulate_ci_job "main - linux x86_64" 3 "x86_64-unknown-linux-gnu"

# Summary
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Simulation Complete${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "${GREEN}Key Observations:${NC}"
echo "  • Job 1: Cold cache - full compilation time"
echo "  • Job 2: Warm cache - significant speedup from shared cache"
echo "  • Job 3: Hot cache - maximum speedup"
echo ""
echo -e "${GREEN}In production CI:${NC}"
echo "  • All runners share the same NativeLink scheduler"
echo "  • Cache persists across all builds (PRs, branches, jobs)"
echo "  • First build of the day warms cache for all subsequent builds"
echo "  • Monorepos benefit massively from shared dependency caching"
echo ""
echo -e "${YELLOW}Full logs saved to:${NC}"
echo "  /tmp/objfs-ci-job-1.log"
echo "  /tmp/objfs-ci-job-2.log"
echo "  /tmp/objfs-ci-job-3.log"
echo ""
