#!/usr/bin/env bash
# =============================================================================
# Run Fen Services Locally Against Docker Infrastructure
# =============================================================================
# This script runs Fen Rust services locally while using Docker for
# infrastructure (Kafka, PostgreSQL, etc.). Useful for rapid development.
#
# Prerequisites:
#   ./scripts/dev-distributed.sh infra  # Start infrastructure first
#
# Usage:
#   ./scripts/run-local.sh api          # Run API in standalone mode
#   ./scripts/run-local.sh api-dist     # Run API in distributed mode
#   ./scripts/run-local.sh coordinator  # Run coordinator
#   ./scripts/run-local.sh data         # Run data node

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

# Common environment variables
export RUST_LOG="${RUST_LOG:-info,fen_api=debug,fen_ingestion=debug,fen_rules=debug}"
export DATABASE_URL="postgres://fen:fen_dev_password@localhost:5432/fen"
export TIMESCALE_URL="postgres://fen_metrics:fen_metrics_password@localhost:5433/fen_metrics"

cmd_api() {
    log_info "Starting Fen API in standalone mode..."

    export BIND_ADDRESS="0.0.0.0:3000"
    export DATABASE_PATH="$PROJECT_ROOT/data/fen.redb"
    export RULES_PATH="$PROJECT_ROOT/rules"
    export REQUIRE_AUTH="false"
    export MAX_UPLOAD_SIZE=52428800
    export ML_ENABLED="${ML_ENABLED:-true}"
    export ML_MODELS_DIR="$PROJECT_ROOT/models"

    mkdir -p "$PROJECT_ROOT/data"

    cd "$PROJECT_ROOT"
    cargo run -p fen-api
}

cmd_api_distributed() {
    log_info "Starting Fen API in distributed mode..."

    export BIND_ADDRESS="0.0.0.0:3000"
    export CLUSTER_MODE="distributed"
    export COORDINATOR_ADDR="http://localhost:9002"
    export KAFKA_BOOTSTRAP_SERVERS="localhost:9092"
    export NUM_SHARDS=2
    export RULES_PATH="$PROJECT_ROOT/rules"
    export REQUIRE_AUTH="false"

    cd "$PROJECT_ROOT"
    cargo run -p fen-api
}

cmd_coordinator() {
    log_info "Starting Fen Coordinator..."

    export NODE_ID="coordinator-local"
    export RAFT_ADDR="0.0.0.0:9001"
    export API_ADDR="0.0.0.0:9002"
    export PEER_ADDRS=""
    export NUM_SHARDS=2
    export REPLICATION_FACTOR=1
    export DATA_DIR="$PROJECT_ROOT/data/coordinator"
    export KAFKA_BOOTSTRAP_SERVERS="localhost:9092"
    export RUST_LOG="info,fen_coordinator=debug,fen_cluster=debug"

    mkdir -p "$DATA_DIR"

    cd "$PROJECT_ROOT"
    cargo run -p fen-coordinator
}

cmd_data() {
    shard_id="${1:-0}"
    port=$((9100 + shard_id))

    log_info "Starting Fen Data Node for shard $shard_id..."

    export NODE_ID="data-node-local-$shard_id"
    export SHARD_IDS="$shard_id"
    export GRPC_ADDR="0.0.0.0:$port"
    export COORDINATOR_ADDR="http://localhost:9002"
    export DATA_DIR="$PROJECT_ROOT/data/data-node-$shard_id"
    export HEARTBEAT_INTERVAL=10
    export ADVERTISED_ADDR="localhost:$port"
    export KAFKA_BOOTSTRAP_SERVERS="localhost:9092"
    export RUST_LOG="info,fen_data=debug,fen_grpc=debug,fen_storage=debug"

    mkdir -p "$DATA_DIR"

    cd "$PROJECT_ROOT"
    cargo run -p fen-data
}

cmd_test() {
    log_info "Running integration tests..."

    export KAFKA_BOOTSTRAP_SERVERS="localhost:9092"
    export NUM_SHARDS=2

    cd "$PROJECT_ROOT"
    cargo test -p fen-tests -- --ignored
}

cmd_help() {
    echo "Run Fen Services Locally"
    echo ""
    echo "Usage: $0 <command> [args]"
    echo ""
    echo "Commands:"
    echo "  api            Run API in standalone mode"
    echo "  api-dist       Run API in distributed mode"
    echo "  coordinator    Run coordinator service"
    echo "  data [shard]   Run data node (default shard: 0)"
    echo "  test           Run integration tests"
    echo "  help           Show this help"
    echo ""
    echo "Prerequisites:"
    echo "  Start infrastructure first: ./scripts/dev-distributed.sh infra"
    echo ""
    echo "Examples:"
    echo "  # Terminal 1: Start coordinator"
    echo "  $0 coordinator"
    echo ""
    echo "  # Terminal 2: Start data node shard 0"
    echo "  $0 data 0"
    echo ""
    echo "  # Terminal 3: Start data node shard 1"
    echo "  $0 data 1"
    echo ""
    echo "  # Terminal 4: Start API in distributed mode"
    echo "  $0 api-dist"
}

case "${1:-help}" in
    api)
        cmd_api
        ;;
    api-dist|api-distributed)
        cmd_api_distributed
        ;;
    coordinator)
        cmd_coordinator
        ;;
    data)
        cmd_data "$2"
        ;;
    test)
        cmd_test
        ;;
    help|--help|-h)
        cmd_help
        ;;
    *)
        echo "Unknown command: $1"
        cmd_help
        exit 1
        ;;
esac
