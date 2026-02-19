#!/bin/bash
# Setup script for Fiber Network testnet with two local nodes
#
# This script:
# 1. Downloads fnn (Fiber Node) and ckb-cli binaries
# 2. Creates two accounts with ckb-cli
# 3. Starts two Fiber nodes connected to testnet
# 4. Opens a channel between the two local nodes
#
# Usage: ./scripts/setup-fiber-testnet.sh

set -e

# Configuration
FNN_VERSION="v0.7.0"
CKB_CLI_VERSION="v2.0.0"
CHANNEL_FUNDING_AMOUNT="0xba43b7400"  # 500 CKB = 50000000000 shannon
CKB_RPC_URL="https://testnet.ckb.dev"
FAUCET_URL="https://faucet.nervos.org"
FAUCET_API_URL="https://faucet-api.nervos.org"
MIN_CAPACITY=100000000000  # 1000 CKB in shannon

# Node ports
NODE_A_RPC_PORT=8227
NODE_A_P2P_PORT=8228
NODE_B_RPC_PORT=8229
NODE_B_P2P_PORT=8230

# Directories
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
WORK_DIR="${PROJECT_DIR}/testnet-fnn"
BIN_DIR="${WORK_DIR}/bin"
CKB_CLI_HOME="${WORK_DIR}/ckb-cli-home"
export CKB_CLI_HOME

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Detect platform
detect_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)
    
    case "$os" in
        linux)
            case "$arch" in
                x86_64) echo "x86_64-linux-portable" ;;
                aarch64) echo "aarch64-linux" ;;
                *) log_error "Unsupported architecture: $arch"; exit 1 ;;
            esac
            ;;
        darwin)
            echo "x86_64-darwin-portable"
            ;;
        *)
            log_error "Unsupported OS: $os"
            exit 1
            ;;
    esac
}

detect_ckb_cli_platform() {
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    local arch=$(uname -m)
    
    case "$os" in
        linux)
            case "$arch" in
                x86_64) echo "x86_64-unknown-linux-gnu" ;;
                aarch64) echo "aarch64-unknown-linux-gnu" ;;
                *) log_error "Unsupported architecture: $arch"; exit 1 ;;
            esac
            ;;
        darwin)
            case "$arch" in
                x86_64) echo "x86_64-apple-darwin" ;;
                arm64) echo "aarch64-apple-darwin" ;;
                *) log_error "Unsupported architecture: $arch"; exit 1 ;;
            esac
            ;;
        *)
            log_error "Unsupported OS: $os"
            exit 1
            ;;
    esac
}

# Download and extract fnn
download_fnn() {
    local platform=$(detect_platform)
    local url="https://github.com/nervosnetwork/fiber/releases/download/${FNN_VERSION}/fnn_${FNN_VERSION}-${platform}.tar.gz"
    
    log_info "Downloading fnn ${FNN_VERSION} for ${platform}..."
    
    mkdir -p "${BIN_DIR}"
    curl -L -o "${BIN_DIR}/fnn.tar.gz" "$url"
    tar -xzf "${BIN_DIR}/fnn.tar.gz" -C "${BIN_DIR}"
    rm "${BIN_DIR}/fnn.tar.gz"
    chmod +x "${BIN_DIR}/fnn"
    
    log_success "fnn downloaded to ${BIN_DIR}/fnn"
}

# Download and extract ckb-cli
download_ckb_cli() {
    local platform=$(detect_ckb_cli_platform)
    local ext="tar.gz"
    local os=$(uname -s | tr '[:upper:]' '[:lower:]')
    
    if [[ "$os" == "darwin" ]]; then
        ext="zip"
    fi
    
    local url="https://github.com/nervosnetwork/ckb-cli/releases/download/${CKB_CLI_VERSION}/ckb-cli_${CKB_CLI_VERSION}_${platform}.${ext}"
    
    log_info "Downloading ckb-cli ${CKB_CLI_VERSION} for ${platform}..."
    
    mkdir -p "${BIN_DIR}"
    
    local extract_dir="ckb-cli_${CKB_CLI_VERSION}_${platform}"
    
    if [[ "$ext" == "zip" ]]; then
        curl -L -o "${BIN_DIR}/ckb-cli.zip" "$url"
        unzip -o "${BIN_DIR}/ckb-cli.zip" -d "${BIN_DIR}"
        rm "${BIN_DIR}/ckb-cli.zip"
    else
        curl -L -o "${BIN_DIR}/ckb-cli.tar.gz" "$url"
        tar -xzf "${BIN_DIR}/ckb-cli.tar.gz" -C "${BIN_DIR}"
        rm "${BIN_DIR}/ckb-cli.tar.gz"
    fi
    
    # Move binary from extracted subdirectory to bin/
    mv "${BIN_DIR}/${extract_dir}/ckb-cli" "${BIN_DIR}/ckb-cli"
    rm -rf "${BIN_DIR}/${extract_dir}"
    chmod +x "${BIN_DIR}/ckb-cli"
    
    log_success "ckb-cli downloaded to ${BIN_DIR}/ckb-cli"
}

# List existing accounts
list_accounts() {
    "${BIN_DIR}/ckb-cli" account list --output-format json 2>/dev/null
}

# Get account count
get_account_count() {
    local accounts=$(list_accounts)
    echo "$accounts" | jq 'length'
}

# Create account using ckb-cli
create_account() {
    local node_name=$1
    local account_index=$2  # 0 for nodeA, 1 for nodeB
    local node_dir="${WORK_DIR}/${node_name}"
    local ckb_dir="${node_dir}/ckb"
    
    mkdir -p "${ckb_dir}"
    
    # Use a fixed password for this demo (in production use a secure password)
    local password="123"
    
    # Check if we need to create a new account
    local account_count=$(get_account_count)
    
    if [[ $account_count -le $account_index ]]; then
        log_info "Creating account for ${node_name}..."
        
        # Create account with ckb-cli
        local output=$("${BIN_DIR}/ckb-cli" account new --local-only <<EOF
${password}
${password}
EOF
2>&1)
        
        # Extract lock_arg from output (macOS compatible)
        local lock_arg=$(echo "$output" | sed -n 's/.*lock_arg:[[:space:]]*\(0x[a-fA-F0-9]*\).*/\1/p' | head -1)
        
        if [[ -z "$lock_arg" ]]; then
            log_error "Failed to create account for ${node_name}"
            echo "$output"
            exit 1
        fi
    else
        log_info "Using existing account for ${node_name}..."
    fi
    
    # Get account info from the list
    local accounts=$(list_accounts)
    local lock_arg=$(echo "$accounts" | jq -r ".[$account_index].lock_arg")
    
    # Check if key file already exists
    if [[ ! -f "${ckb_dir}/key" ]]; then
        log_info "Exporting private key for ${node_name}..."
        
        # Export private key
        "${BIN_DIR}/ckb-cli" account export --lock-arg "$lock_arg" --extended-privkey-path "${ckb_dir}/exported-key" <<EOF
${password}
EOF
        
        # Extract first line (the actual private key) and save to key file
        head -n 1 "${ckb_dir}/exported-key" > "${ckb_dir}/key"
        chmod 600 "${ckb_dir}/key"
        rm -f "${ckb_dir}/exported-key"
    fi
    
    # Get address using key-info (macOS compatible)
    local key_info=$("${BIN_DIR}/ckb-cli" util key-info --privkey-path "${ckb_dir}/key" 2>&1)
    local address=$(echo "$key_info" | sed -n 's/.*testnet:[[:space:]]*\([^[:space:]]*\).*/\1/p' | head -1)
    
    # Save address to file for reference
    echo "$address" > "${ckb_dir}/address"
    echo "$lock_arg" > "${ckb_dir}/lock_arg"
    
    log_success "Account ready for ${node_name}"
    echo "  Address: ${address}"
    echo "  Lock arg: ${lock_arg}"
}

# Check account balance
check_balance() {
    local address=$1
    local result=$("${BIN_DIR}/ckb-cli" --url "$CKB_RPC_URL" wallet get-capacity --address "$address" 2>&1)
    # Extract total capacity in CKB (e.g., "1000.0") - macOS compatible
    local capacity=$(echo "$result" | sed -n 's/.*total:[[:space:]]*\([0-9.]*\).*/\1/p' | head -1)
    echo "$capacity"
}

# Claim CKB from faucet API
# Args: address, amount (10000, 100000, or 300000)
claim_from_faucet() {
    local address=$1
    local amount=${2:-10000}
    
    local response=$(curl -s -X POST "${FAUCET_API_URL}/claim_events" \
        -H "Content-Type: application/json" \
        -H "User-Agent: Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36" \
        -H "Origin: https://faucet.nervos.org" \
        -H "Referer: https://faucet.nervos.org/" \
        -d "{\"claim_event\": {\"address_hash\": \"${address}\", \"amount\": \"${amount}\"}}")
    
    # Check for rate limiting (Cloudflare error 1015)
    if echo "$response" | grep -q "error code: 1015"; then
        echo "error: rate limited, please try again later"
    # Check if response contains error
    elif echo "$response" | grep -q '"errors"'; then
        local error=$(echo "$response" | sed -n 's/.*"errors":[[:space:]]*{[[:space:]]*"[^"]*":[[:space:]]*"\([^"]*\)".*/\1/p')
        if [[ -n "$error" ]]; then
            echo "error: $error"
        else
            echo "error: unknown"
        fi
    elif echo "$response" | grep -q '"data"'; then
        echo "success"
    else
        echo "error: ${response:0:100}"
    fi
}

# Wait for both accounts to have sufficient balance
wait_for_funding() {
    local nodeA_address=$(cat "${WORK_DIR}/nodeA/ckb/address")
    local nodeB_address=$(cat "${WORK_DIR}/nodeB/ckb/address")
    local min_ckb=1000
    local claim_amount=10000  # Claim 10000 CKB per request
    local balance_check_interval=10  # Check balance every 10 seconds
    local claim_retry_interval=60    # Retry faucet claim every 60 seconds
    local last_claim_time=0
    
    echo ""
    echo "=========================================="
    echo "  Auto-claiming CKB from Faucet"
    echo "=========================================="
    echo ""
    echo -e "${GREEN}NodeA Address:${NC} $nodeA_address"
    echo -e "${GREEN}NodeB Address:${NC} $nodeB_address"
    echo ""
    
    log_info "Waiting for balances (need >= ${min_ckb} CKB each)..."
    log_info "Will auto-claim from faucet every ${claim_retry_interval}s if needed."
    echo ""
    
    while true; do
        local current_time=$(date +%s)
        
        # Check balances
        local balanceA=$(check_balance "$nodeA_address")
        local balanceB=$(check_balance "$nodeB_address")
        
        # Default to 0 if empty
        balanceA=${balanceA:-0}
        balanceB=${balanceB:-0}
        
        # Remove decimal part for comparison
        local intA=$(echo "$balanceA" | cut -d. -f1)
        local intB=$(echo "$balanceB" | cut -d. -f1)
        intA=${intA:-0}
        intB=${intB:-0}
        
        echo -ne "\r  NodeA: ${balanceA} CKB | NodeB: ${balanceB} CKB    "
        
        # Check if both funded
        if [[ $intA -ge $min_ckb ]] && [[ $intB -ge $min_ckb ]]; then
            echo ""
            log_success "Both accounts funded!"
            return 0
        fi
        
        # Try to claim if interval has passed and balance is insufficient
        local time_since_claim=$((current_time - last_claim_time))
        if [[ $time_since_claim -ge $claim_retry_interval ]]; then
            echo ""
            
            if [[ $intA -lt $min_ckb ]]; then
                log_info "Claiming ${claim_amount} CKB for NodeA..."
                local resultA=$(claim_from_faucet "$nodeA_address" "$claim_amount")
                if [[ "$resultA" == "success" ]]; then
                    log_success "Faucet claim submitted for NodeA"
                else
                    log_warn "NodeA faucet: ${resultA#error: }"
                fi
            fi
            
            if [[ $intB -lt $min_ckb ]]; then
                log_info "Claiming ${claim_amount} CKB for NodeB..."
                local resultB=$(claim_from_faucet "$nodeB_address" "$claim_amount")
                if [[ "$resultB" == "success" ]]; then
                    log_success "Faucet claim submitted for NodeB"
                else
                    log_warn "NodeB faucet: ${resultB#error: }"
                fi
            fi
            
            last_claim_time=$current_time
            echo ""
        fi
        
        sleep $balance_check_interval
    done
}

# Create config file for a node
create_config() {
    local node_name=$1
    local rpc_port=$2
    local p2p_port=$3
    local node_dir="${WORK_DIR}/${node_name}"
    
    log_info "Creating config for ${node_name}..."
    
    cat > "${node_dir}/config.yml" << EOF
fiber:
  listening_addr: "/ip4/0.0.0.0/tcp/${p2p_port}"
  bootnode_addrs:
    - "/ip4/54.179.226.154/tcp/8228/p2p/Qmes1EBD4yNo9Ywkfe6eRw9tG1nVNGLDmMud1xJMsoYFKy"
    - "/ip4/16.163.7.105/tcp/8228/p2p/QmdyQWjPtbK4NWWsvy8s69NGJaQULwgeQDT5ZpNDrTNaeV"
  announce_listening_addr: false
  # Auto accept channels with at least 100 CKB funding
  open_channel_auto_accept_min_ckb_funding_amount: 10000000000
  # Auto contribute 500 CKB when accepting a channel
  auto_accept_channel_ckb_funding_amount: 50000000000
  chain: testnet
  scripts:
    - name: FundingLock
      script:
        code_hash: 0x6c67887fe201ee0c7853f1682c0b77c0e6214044c156c7558269390a8afa6d7c
        hash_type: type
        args: 0x
      cell_deps:
        - type_id:
            code_hash: 0x00000000000000000000000000000000000000000000000000545950455f4944
            hash_type: type
            args: 0x3cb7c0304fe53f75bb5727e2484d0beae4bd99d979813c6fc97c3cca569f10f6
        - cell_dep:
            out_point:
              tx_hash: 0x12c569a258dd9c5bd99f632bb8314b1263b90921ba31496467580d6b79dd14a7
              index: 0x0
            dep_type: code
    - name: CommitmentLock
      script:
        code_hash: 0x740dee83f87c6f309824d8fd3fbdd3c8380ee6fc9acc90b1a748438afcdf81d8
        hash_type: type
        args: 0x
      cell_deps:
        - type_id:
            code_hash: 0x00000000000000000000000000000000000000000000000000545950455f4944
            hash_type: type
            args: 0xf7e458887495cf70dd30d1543cad47dc1dfe9d874177bf19291e4db478d5751b
        - cell_dep:
            out_point:
              tx_hash: 0x12c569a258dd9c5bd99f632bb8314b1263b90921ba31496467580d6b79dd14a7
              index: 0x0
            dep_type: code

rpc:
  listening_addr: "127.0.0.1:${rpc_port}"

ckb:
  rpc_url: "https://testnet.ckbapp.dev/"
  udt_whitelist:
    - name: RUSD
      script:
        code_hash: 0x1142755a044bf2ee358cba9f2da187ce928c91cd4dc8692ded0337efa677d21a
        hash_type: type
        args: 0x878fcc6f1f08d48e87bb1c3b3d5083f23f8a39c5d5c764f253b55b998526439b
      cell_deps:
        - type_id:
            code_hash: 0x00000000000000000000000000000000000000000000000000545950455f4944
            hash_type: type
            args: 0x97d30b723c0b2c66e9cb8d4d0df4ab5d7222cbb00d4a9a2055ce2e5d7f0d8b0f
      auto_accept_amount: 1000000000

services:
  - fiber
  - rpc
  - ckb
EOF

    log_success "Config created at ${node_dir}/config.yml"
}

# Start a fiber node
start_node() {
    local node_name=$1
    local node_dir="${WORK_DIR}/${node_name}"
    local password="123"  # Demo password, use a strong one in production
    
    log_info "Starting ${node_name}..."
    
    FIBER_SECRET_KEY_PASSWORD="${password}" RUST_LOG=info \
        "${BIN_DIR}/fnn" -c "${node_dir}/config.yml" -d "${node_dir}" \
        > "${node_dir}/fnn.log" 2>&1 &
    
    local pid=$!
    echo "$pid" > "${node_dir}/fnn.pid"
    
    log_success "${node_name} started with PID ${pid}"
}

# Wait for node to be ready
wait_for_node() {
    local node_name=$1
    local rpc_port=$2
    local max_attempts=60
    local attempt=0
    
    log_info "Waiting for ${node_name} to be ready..."
    
    while [[ $attempt -lt $max_attempts ]]; do
        if curl -s "http://127.0.0.1:${rpc_port}" > /dev/null 2>&1; then
            log_success "${node_name} is ready"
            return 0
        fi
        sleep 1
        attempt=$((attempt + 1))
    done
    
    log_error "${node_name} failed to start within ${max_attempts} seconds"
    return 1
}

# Connect to peer
connect_peer() {
    local rpc_port=$1
    local peer_addr=$2
    
    curl -s --location "http://127.0.0.1:${rpc_port}" \
        --header 'Content-Type: application/json' \
        --data '{
            "id": 1,
            "jsonrpc": "2.0",
            "method": "connect_peer",
            "params": [
                {
                    "address": "'"${peer_addr}"'"
                }
            ]
        }'
}

# Open channel
open_channel() {
    local rpc_port=$1
    local peer_id=$2
    local funding_amount=$3
    
    curl -s --location "http://127.0.0.1:${rpc_port}" \
        --header 'Content-Type: application/json' \
        --data '{
            "id": 2,
            "jsonrpc": "2.0",
            "method": "open_channel",
            "params": [
                {
                    "peer_id": "'"${peer_id}"'",
                    "funding_amount": "'"${funding_amount}"'",
                    "public": true
                }
            ]
        }'
}

# List channels
list_channels() {
    local rpc_port=$1
    local peer_id=${2:-}
    
    if [[ -n "$peer_id" ]]; then
        curl -s --location "http://127.0.0.1:${rpc_port}" \
            --header 'Content-Type: application/json' \
            --data '{
                "id": 3,
                "jsonrpc": "2.0",
                "method": "list_channels",
                "params": [
                    {
                        "peer_id": "'"${peer_id}"'"
                    }
                ]
            }'
    else
        curl -s --location "http://127.0.0.1:${rpc_port}" \
            --header 'Content-Type: application/json' \
            --data '{
                "id": 3,
                "jsonrpc": "2.0",
                "method": "list_channels",
                "params": [{}]
            }'
    fi
}

# Get node info (peer_id)
get_node_info() {
    local rpc_port=$1
    curl -s --location "http://127.0.0.1:${rpc_port}" \
        --header 'Content-Type: application/json' \
        --data '{
            "id": 4,
            "jsonrpc": "2.0",
            "method": "node_info",
            "params": []
        }'
}

# Extract peer_id from node log file (macOS compatible)
get_peer_id_from_log() {
    local node_dir=$1
    sed -n 's/.*peer id PeerId(\([^)]*\)).*/\1/p' "${node_dir}/fnn.log" | head -1
}

# Wait for channel to be ready
wait_for_channel_ready() {
    local node_name=$1
    local rpc_port=$2
    local peer_id=$3
    local max_attempts=120
    local attempt=0
    
    log_info "Waiting for ${node_name} channel to be ready..."
    
    while [[ $attempt -lt $max_attempts ]]; do
        local result=$(list_channels "$rpc_port" "$peer_id")
        if echo "$result" | grep -q "CHANNEL_READY"; then
            log_success "${node_name} channel is ready!"
            return 0
        fi
        sleep 5
        attempt=$((attempt + 1))
        echo -n "."
    done
    
    echo ""
    log_warn "${node_name} channel not ready yet. Check status manually."
    return 1
}

# Stop nodes
stop_nodes() {
    log_info "Stopping nodes..."
    
    for node_name in nodeA nodeB; do
        local pid_file="${WORK_DIR}/${node_name}/fnn.pid"
        if [[ -f "$pid_file" ]]; then
            local pid=$(cat "$pid_file")
            if kill -0 "$pid" 2>/dev/null; then
                kill "$pid"
                log_success "Stopped ${node_name} (PID ${pid})"
            fi
            rm "$pid_file"
        fi
    done
}

# Main setup function
main() {
    echo ""
    echo "=========================================="
    echo "  Fiber Network Testnet Setup Script"
    echo "=========================================="
    echo ""
    
    # Create work directory
    mkdir -p "${WORK_DIR}"
    
    # Download binaries if not present
    if [[ ! -x "${BIN_DIR}/fnn" ]]; then
        download_fnn
    else
        log_info "fnn already exists, skipping download"
    fi
    
    if [[ ! -x "${BIN_DIR}/ckb-cli" ]]; then
        download_ckb_cli
    else
        log_info "ckb-cli already exists, skipping download"
    fi
    
    # Check if nodes already exist
    if [[ -d "${WORK_DIR}/nodeA/ckb" ]] && [[ -d "${WORK_DIR}/nodeB/ckb" ]]; then
        log_warn "Node directories already exist. Do you want to recreate them? (y/N)"
        read -r response
        if [[ "$response" != "y" && "$response" != "Y" ]]; then
            log_info "Using existing node configuration"
        else
            rm -rf "${WORK_DIR}/nodeA" "${WORK_DIR}/nodeB"
            create_account "nodeA" 0
            create_account "nodeB" 1
            create_config "nodeA" "$NODE_A_RPC_PORT" "$NODE_A_P2P_PORT"
            create_config "nodeB" "$NODE_B_RPC_PORT" "$NODE_B_P2P_PORT"
        fi
    else
        # Create accounts (will reuse existing accounts in CKB_CLI_HOME if available)
        create_account "nodeA" 0
        create_account "nodeB" 1
        
        # Create configs
        create_config "nodeA" "$NODE_A_RPC_PORT" "$NODE_A_P2P_PORT"
        create_config "nodeB" "$NODE_B_RPC_PORT" "$NODE_B_P2P_PORT"
    fi
    
    # Wait for accounts to be funded
    wait_for_funding
    
    # Start nodes
    start_node "nodeA"
    start_node "nodeB"
    
    # Wait for nodes to be ready
    wait_for_node "nodeA" "$NODE_A_RPC_PORT"
    wait_for_node "nodeB" "$NODE_B_RPC_PORT"
    
    # Give nodes a moment to fully initialize and write peer_id to log
    sleep 2
    
    # Get peer IDs
    echo ""
    log_info "Getting node peer IDs..."
    
    local nodeA_peer_id=$(get_peer_id_from_log "${WORK_DIR}/nodeA")
    local nodeB_peer_id=$(get_peer_id_from_log "${WORK_DIR}/nodeB")
    
    if [[ -z "$nodeA_peer_id" ]] || [[ -z "$nodeB_peer_id" ]]; then
        log_error "Failed to get peer IDs from logs"
        log_error "NodeA log tail:"
        tail -10 "${WORK_DIR}/nodeA/fnn.log"
        log_error "NodeB log tail:"
        tail -10 "${WORK_DIR}/nodeB/fnn.log"
        exit 1
    fi
    
    echo "  NodeA peer_id: ${nodeA_peer_id}"
    echo "  NodeB peer_id: ${nodeB_peer_id}"
    
    # Save peer IDs for status command
    echo "$nodeA_peer_id" > "${WORK_DIR}/nodeA/peer_id"
    echo "$nodeB_peer_id" > "${WORK_DIR}/nodeB/peer_id"
    
    # Connect NodeA to NodeB
    echo ""
    log_info "Connecting NodeA to NodeB..."
    
    local nodeB_addr="/ip4/127.0.0.1/tcp/${NODE_B_P2P_PORT}/p2p/${nodeB_peer_id}"
    connect_peer "$NODE_A_RPC_PORT" "$nodeB_addr"
    sleep 2
    
    # Open channel from NodeA to NodeB
    echo ""
    log_info "Opening 500 CKB channel from NodeA to NodeB..."
    
    local result=$(open_channel "$NODE_A_RPC_PORT" "$nodeB_peer_id" "$CHANNEL_FUNDING_AMOUNT")
    echo "open_channel result: $result"
    
    # Wait for channel to be ready
    echo ""
    log_info "Waiting for channel to be ready (this may take a few minutes)..."
    
    wait_for_channel_ready "nodeA" "$NODE_A_RPC_PORT" "$nodeB_peer_id"
    
    echo ""
    echo "=========================================="
    echo "  Setup Complete!"
    echo "=========================================="
    echo ""
    echo "NodeA RPC: http://127.0.0.1:${NODE_A_RPC_PORT}"
    echo "NodeB RPC: http://127.0.0.1:${NODE_B_RPC_PORT}"
    echo ""
    echo "NodeA peer_id: ${nodeA_peer_id}"
    echo "NodeB peer_id: ${nodeB_peer_id}"
    echo ""
    echo "To check channel status:"
    echo "  $0 status"
    echo ""
    echo "To stop nodes:"
    echo "  $0 stop"
    echo ""
    echo "Logs:"
    echo "  ${WORK_DIR}/nodeA/fnn.log"
    echo "  ${WORK_DIR}/nodeB/fnn.log"
    echo ""
}

# Handle commands
case "${1:-}" in
    stop)
        stop_nodes
        ;;
    status)
        echo "NodeA channels:"
        list_channels "$NODE_A_RPC_PORT" | jq .
        echo ""
        echo "NodeB channels:"
        list_channels "$NODE_B_RPC_PORT" | jq .
        ;;
    *)
        main
        ;;
esac
