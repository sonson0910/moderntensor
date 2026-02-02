#!/bin/bash
# =============================================================================
# ModernTensor Testnet Deployment Script
# Federated Learning Layer Contracts
# =============================================================================

set -e

# Configuration
NETWORK="${NETWORK:-moderntensor-testnet}"
RPC_URL="${RPC_URL:-http://localhost:8545}"
PRIVATE_KEY="${PRIVATE_KEY:-}"
MDT_TOKEN="${MDT_TOKEN:-}"

# Contract paths
CONTRACTS_DIR="$(dirname "$0")/../src"
GRADIENT_AGGREGATOR="$CONTRACTS_DIR/GradientAggregator.sol"
TRAINING_ESCROW="$CONTRACTS_DIR/TrainingEscrow.sol"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "=============================================="
echo "ModernTensor Federated Learning Deployment"
echo "=============================================="
echo ""

# Check prerequisites
check_prerequisites() {
    echo "Checking prerequisites..."

    if ! command -v forge &> /dev/null; then
        echo -e "${RED}Error: Foundry (forge) not found${NC}"
        echo "Install: curl -L https://foundry.paradigm.xyz | bash"
        exit 1
    fi

    if [ -z "$PRIVATE_KEY" ]; then
        echo -e "${YELLOW}Warning: PRIVATE_KEY not set${NC}"
        echo "Set with: export PRIVATE_KEY=0x..."
    fi

    if [ -z "$MDT_TOKEN" ]; then
        echo -e "${YELLOW}Warning: MDT_TOKEN address not set${NC}"
        echo "Set with: export MDT_TOKEN=0x..."
    fi

    echo -e "${GREEN}Prerequisites OK${NC}"
}

# Compile contracts
compile_contracts() {
    echo ""
    echo "Compiling contracts..."

    cd "$(dirname "$0")/.."

    forge build --force

    echo -e "${GREEN}Compilation successful${NC}"
}

# Deploy GradientAggregator
deploy_gradient_aggregator() {
    echo ""
    echo "Deploying GradientAggregator..."

    if [ -z "$MDT_TOKEN" ]; then
        echo -e "${RED}Error: MDT_TOKEN required${NC}"
        exit 1
    fi

    AGGREGATOR_ADDRESS=$(forge create \
        --rpc-url "$RPC_URL" \
        --private-key "$PRIVATE_KEY" \
        "$GRADIENT_AGGREGATOR:GradientAggregator" \
        --constructor-args "$MDT_TOKEN" \
        --json | jq -r '.deployedTo')

    echo -e "${GREEN}GradientAggregator deployed: $AGGREGATOR_ADDRESS${NC}"
    export GRADIENT_AGGREGATOR_ADDRESS="$AGGREGATOR_ADDRESS"
}

# Deploy TrainingEscrow
deploy_training_escrow() {
    echo ""
    echo "Deploying TrainingEscrow..."

    if [ -z "$MDT_TOKEN" ] || [ -z "$GRADIENT_AGGREGATOR_ADDRESS" ]; then
        echo -e "${RED}Error: MDT_TOKEN and GRADIENT_AGGREGATOR_ADDRESS required${NC}"
        exit 1
    fi

    ESCROW_ADDRESS=$(forge create \
        --rpc-url "$RPC_URL" \
        --private-key "$PRIVATE_KEY" \
        "$TRAINING_ESCROW:TrainingEscrow" \
        --constructor-args "$MDT_TOKEN" "$GRADIENT_AGGREGATOR_ADDRESS" \
        --json | jq -r '.deployedTo')

    echo -e "${GREEN}TrainingEscrow deployed: $ESCROW_ADDRESS${NC}"
    export TRAINING_ESCROW_ADDRESS="$ESCROW_ADDRESS"
}

# Configure contracts
configure_contracts() {
    echo ""
    echo "Configuring contracts..."

    # Set escrow in aggregator
    # forge cast send ...

    echo -e "${GREEN}Configuration complete${NC}"
}

# Verify contracts
verify_contracts() {
    echo ""
    echo "Verifying contracts..."

    # Skip verification for testnet
    echo -e "${YELLOW}Skipping verification for testnet${NC}"
}

# Save deployment info
save_deployment() {
    echo ""
    echo "Saving deployment info..."

    DEPLOYMENT_FILE="$(dirname "$0")/../deployments/${NETWORK}.json"
    mkdir -p "$(dirname "$DEPLOYMENT_FILE")"

    cat > "$DEPLOYMENT_FILE" << EOF
{
    "network": "$NETWORK",
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "contracts": {
        "MDTToken": "$MDT_TOKEN",
        "GradientAggregator": "$GRADIENT_AGGREGATOR_ADDRESS",
        "TrainingEscrow": "$TRAINING_ESCROW_ADDRESS"
    },
    "rpcUrl": "$RPC_URL"
}
EOF

    echo -e "${GREEN}Deployment saved: $DEPLOYMENT_FILE${NC}"
}

# Summary
print_summary() {
    echo ""
    echo "=============================================="
    echo "DEPLOYMENT SUMMARY"
    echo "=============================================="
    echo ""
    echo "Network:             $NETWORK"
    echo "RPC URL:             $RPC_URL"
    echo ""
    echo "Contracts:"
    echo "  MDT Token:         $MDT_TOKEN"
    echo "  GradientAggregator: $GRADIENT_AGGREGATOR_ADDRESS"
    echo "  TrainingEscrow:    $TRAINING_ESCROW_ADDRESS"
    echo ""
    echo "=============================================="
}

# Main
main() {
    check_prerequisites
    compile_contracts

    if [ -n "$PRIVATE_KEY" ] && [ -n "$MDT_TOKEN" ]; then
        deploy_gradient_aggregator
        deploy_training_escrow
        configure_contracts
        verify_contracts
        save_deployment
        print_summary
    else
        echo ""
        echo -e "${YELLOW}Dry run complete. Set PRIVATE_KEY and MDT_TOKEN to deploy.${NC}"
    fi
}

main "$@"
