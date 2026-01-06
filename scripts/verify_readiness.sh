#!/bin/bash
# LuxTensor Layer 1 Readiness Verification Script
# Kiểm tra xem blockchain đã sẵn sàng triển khai testnet chưa

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LUXTENSOR_DIR="$REPO_ROOT/luxtensor"

PASSED=0
FAILED=0
WARNINGS=0

print_header() {
    echo ""
    echo -e "${BLUE}================================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}================================================${NC}"
}

print_check() {
    echo -n "Checking $1... "
}

print_pass() {
    echo -e "${GREEN}✓ PASS${NC}"
    ((PASSED++))
}

print_fail() {
    echo -e "${RED}✗ FAIL${NC}"
    echo -e "${RED}  → $1${NC}"
    ((FAILED++))
}

print_warn() {
    echo -e "${YELLOW}⚠ WARNING${NC}"
    echo -e "${YELLOW}  → $1${NC}"
    ((WARNINGS++))
}

check_rust() {
    print_header "1. Rust Toolchain"
    
    print_check "Rust installation"
    if command -v rustc &> /dev/null; then
        RUST_VERSION=$(rustc --version | cut -d' ' -f2)
        print_pass
        echo "  Version: $RUST_VERSION"
    else
        print_fail "Rust not installed"
    fi
    
    print_check "Cargo installation"
    if command -v cargo &> /dev/null; then
        CARGO_VERSION=$(cargo --version | cut -d' ' -f2)
        print_pass
        echo "  Version: $CARGO_VERSION"
    else
        print_fail "Cargo not installed"
    fi
}

check_luxtensor_structure() {
    print_header "2. LuxTensor Repository Structure"
    
    print_check "luxtensor directory exists"
    if [ -d "$LUXTENSOR_DIR" ]; then
        print_pass
    else
        print_fail "luxtensor directory not found"
        return
    fi
    
    print_check "Cargo workspace configuration"
    if [ -f "$LUXTENSOR_DIR/Cargo.toml" ]; then
        print_pass
    else
        print_fail "Cargo.toml not found"
    fi
    
    # Check crates
    CRATES=(
        "luxtensor-core"
        "luxtensor-crypto"
        "luxtensor-consensus"
        "luxtensor-network"
        "luxtensor-storage"
        "luxtensor-rpc"
        "luxtensor-node"
        "luxtensor-cli"
    )
    
    for crate in "${CRATES[@]}"; do
        print_check "crate: $crate"
        if [ -d "$LUXTENSOR_DIR/crates/$crate" ]; then
            print_pass
        else
            print_fail "Crate $crate not found"
        fi
    done
}

check_build() {
    print_header "3. Build Status"
    
    print_check "Building luxtensor workspace"
    cd "$LUXTENSOR_DIR"
    
    if cargo check --workspace &> /tmp/luxtensor_build.log; then
        print_pass
    else
        print_fail "Build failed. Check /tmp/luxtensor_build.log"
        return
    fi
    
    print_check "Node binary compilation"
    if cargo build --release -p luxtensor-node &> /tmp/luxtensor_node_build.log; then
        print_pass
        if [ -f "target/release/luxtensor-node" ]; then
            SIZE=$(du -h target/release/luxtensor-node | cut -f1)
            echo "  Binary size: $SIZE"
        fi
    else
        print_fail "Node binary build failed. Check /tmp/luxtensor_node_build.log"
    fi
}

check_tests() {
    print_header "4. Test Suite"
    
    cd "$LUXTENSOR_DIR"
    
    print_check "Core tests"
    if cargo test -p luxtensor-core --lib &> /tmp/core_tests.log; then
        PASSED_TESTS=$(grep "test result:" /tmp/core_tests.log | grep -o "[0-9]* passed" | grep -o "[0-9]*")
        print_pass
        echo "  Tests passed: $PASSED_TESTS"
    else
        print_fail "Core tests failed"
    fi
    
    print_check "Crypto tests"
    if cargo test -p luxtensor-crypto --lib &> /tmp/crypto_tests.log; then
        PASSED_TESTS=$(grep "test result:" /tmp/crypto_tests.log | grep -o "[0-9]* passed" | grep -o "[0-9]*")
        print_pass
        echo "  Tests passed: $PASSED_TESTS"
    else
        print_fail "Crypto tests failed"
    fi
    
    print_check "Consensus tests"
    if cargo test -p luxtensor-consensus --lib &> /tmp/consensus_tests.log; then
        PASSED_TESTS=$(grep "test result:" /tmp/consensus_tests.log | grep -o "[0-9]* passed" | grep -o "[0-9]*")
        print_pass
        echo "  Tests passed: $PASSED_TESTS"
    else
        print_fail "Consensus tests failed"
    fi
}

check_configs() {
    print_header "5. Configuration Files"
    
    print_check "Example config exists"
    if [ -f "$LUXTENSOR_DIR/config.example.toml" ]; then
        print_pass
    else
        print_warn "Example config not found"
    fi
    
    print_check "Testnet config exists"
    if [ -f "$LUXTENSOR_DIR/config.testnet.toml" ]; then
        print_pass
    else
        print_warn "Testnet config not found"
    fi
    
    print_check "Genesis config exists"
    if [ -f "$LUXTENSOR_DIR/genesis.testnet.json" ]; then
        print_pass
    else
        print_warn "Genesis config not found"
    fi
}

check_deployment_files() {
    print_header "6. Deployment Files"
    
    print_check "Dockerfile for Rust"
    if [ -f "$LUXTENSOR_DIR/Dockerfile.rust" ]; then
        print_pass
    else
        print_warn "Rust Dockerfile not found"
    fi
    
    print_check "Deployment script"
    if [ -f "$REPO_ROOT/scripts/deploy_testnet.sh" ]; then
        print_pass
        if [ -x "$REPO_ROOT/scripts/deploy_testnet.sh" ]; then
            echo "  Executable: Yes"
        else
            echo "  Executable: No"
        fi
    else
        print_warn "Deployment script not found"
    fi
    
    print_check "Docker compose file"
    if [ -f "$REPO_ROOT/docker/docker-compose.yml" ]; then
        print_pass
    else
        print_warn "Docker compose not found"
    fi
    
    print_check "Kubernetes manifests"
    if [ -d "$REPO_ROOT/k8s" ] && [ "$(ls -A $REPO_ROOT/k8s)" ]; then
        print_pass
        echo "  Files: $(ls $REPO_ROOT/k8s | wc -l)"
    else
        print_warn "Kubernetes manifests not found or empty"
    fi
}

check_documentation() {
    print_header "7. Documentation"
    
    print_check "README exists"
    if [ -f "$LUXTENSOR_DIR/README.md" ]; then
        print_pass
    else
        print_warn "README not found"
    fi
    
    print_check "Testnet deployment guide"
    if [ -f "$REPO_ROOT/TESTNET_DEPLOYMENT_GUIDE.md" ]; then
        print_pass
    else
        print_warn "Testnet deployment guide not found"
    fi
    
    print_check "LuxTensor completion docs"
    if [ -f "$REPO_ROOT/LUXTENSOR_FINAL_COMPLETION.md" ]; then
        print_pass
    else
        print_warn "Completion docs not found"
    fi
}

check_features() {
    print_header "8. Core Features Verification"
    
    cd "$LUXTENSOR_DIR"
    
    print_check "Block structure"
    if grep -r "struct Block" crates/luxtensor-core/src/ &> /dev/null; then
        print_pass
    else
        print_fail "Block structure not found"
    fi
    
    print_check "Transaction format"
    if grep -r "struct Transaction" crates/luxtensor-core/src/ &> /dev/null; then
        print_pass
    else
        print_fail "Transaction structure not found"
    fi
    
    print_check "Proof of Stake consensus"
    if grep -r "struct ProofOfStake" crates/luxtensor-consensus/src/ &> /dev/null; then
        print_pass
    else
        print_fail "PoS consensus not found"
    fi
    
    print_check "P2P networking"
    if grep -r "struct P2PNode" crates/luxtensor-network/src/ &> /dev/null; then
        print_pass
    else
        print_fail "P2P node not found"
    fi
    
    print_check "RPC server"
    if grep -r "struct RpcServer" crates/luxtensor-rpc/src/ &> /dev/null; then
        print_pass
    else
        print_fail "RPC server not found"
    fi
    
    print_check "Storage layer"
    if grep -r "struct BlockchainDB" crates/luxtensor-storage/src/ &> /dev/null; then
        print_pass
    else
        print_fail "Storage layer not found"
    fi
}

generate_report() {
    print_header "VERIFICATION SUMMARY"
    
    TOTAL=$((PASSED + FAILED + WARNINGS))
    PASS_RATE=$((PASSED * 100 / TOTAL))
    
    echo ""
    echo -e "Total Checks: $TOTAL"
    echo -e "${GREEN}Passed: $PASSED${NC}"
    echo -e "${RED}Failed: $FAILED${NC}"
    echo -e "${YELLOW}Warnings: $WARNINGS${NC}"
    echo ""
    echo -e "Pass Rate: ${GREEN}${PASS_RATE}%${NC}"
    echo ""
    
    if [ $FAILED -eq 0 ]; then
        echo -e "${GREEN}================================================${NC}"
        echo -e "${GREEN}✅ LuxTensor Layer 1 is READY for Testnet!${NC}"
        echo -e "${GREEN}================================================${NC}"
        echo ""
        echo "Next steps:"
        echo "1. Review testnet configuration: luxtensor/config.testnet.toml"
        echo "2. Initialize testnet: ./scripts/deploy_testnet.sh init"
        echo "3. Start testnet: ./scripts/deploy_testnet.sh start"
        echo "4. Check status: ./scripts/deploy_testnet.sh status"
        echo ""
        return 0
    else
        echo -e "${RED}================================================${NC}"
        echo -e "${RED}❌ LuxTensor Layer 1 has issues to fix${NC}"
        echo -e "${RED}================================================${NC}"
        echo ""
        echo "Please address the failed checks above before deploying testnet."
        echo ""
        return 1
    fi
}

# Main execution
echo ""
echo -e "${BLUE}╔════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  LuxTensor Layer 1 Readiness Verification     ║${NC}"
echo -e "${BLUE}║  Kiểm tra sẵn sàng triển khai Testnet         ║${NC}"
echo -e "${BLUE}╚════════════════════════════════════════════════╝${NC}"

check_rust
check_luxtensor_structure
check_build
check_tests
check_configs
check_deployment_files
check_documentation
check_features
generate_report

exit $?
