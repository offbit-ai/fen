#!/usr/bin/env bash
# =============================================================================
# Fen Distributed Development Environment
# =============================================================================
# This script manages the local distributed infrastructure for development.
#
# Usage:
#   ./scripts/dev-distributed.sh start     # Start all services
#   ./scripts/dev-distributed.sh stop      # Stop all services
#   ./scripts/dev-distributed.sh restart   # Restart all services
#   ./scripts/dev-distributed.sh status    # Show status
#   ./scripts/dev-distributed.sh logs      # Tail all logs
#   ./scripts/dev-distributed.sh logs api  # Tail specific service logs
#   ./scripts/dev-distributed.sh infra     # Start infrastructure only
#   ./scripts/dev-distributed.sh build     # Build Fen images
#   ./scripts/dev-distributed.sh clean     # Stop and remove volumes
#   ./scripts/dev-distributed.sh shell     # Open shell in api container

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
COMPOSE_FILE="$PROJECT_ROOT/deploy/docker/docker-compose.distributed.yml"
ENV_FILE="$PROJECT_ROOT/deploy/docker/.env.distributed"

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
    echo -e "${GREEN}[OK]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_docker() {
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed. Please install Docker first."
        exit 1
    fi

    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running. Please start Docker."
        exit 1
    fi
}

setup_env() {
    if [ ! -f "$ENV_FILE" ]; then
        log_warn "Environment file not found. Creating from example..."
        cp "$PROJECT_ROOT/deploy/docker/.env.distributed.example" "$ENV_FILE"
        log_success "Created $ENV_FILE"
    fi
}

compose() {
    docker compose -f "$COMPOSE_FILE" --env-file "$ENV_FILE" "$@"
}

cmd_start() {
    check_docker
    setup_env

    log_info "Starting Fen distributed infrastructure..."
    compose up -d

    log_info "Waiting for services to be healthy..."
    sleep 5

    log_success "Distributed environment started!"
    echo ""
    echo "Access points:"
    echo "  - Fen API:        http://localhost:3000"
    echo "  - Kafka Console:  http://localhost:8082"
    echo "  - Coordinator:    http://localhost:9002"
    echo "  - PostgreSQL:     localhost:5432 (fen/fen_dev_password)"
    echo "  - TimescaleDB:    localhost:5433 (fen_metrics/fen_metrics_password)"
    echo ""
    echo "Use './scripts/dev-distributed.sh logs' to view logs"
}

cmd_stop() {
    log_info "Stopping Fen distributed infrastructure..."
    compose down
    log_success "Stopped"
}

cmd_restart() {
    cmd_stop
    cmd_start
}

cmd_status() {
    log_info "Service status:"
    compose ps
}

cmd_logs() {
    if [ -n "$1" ]; then
        compose logs -f "$1"
    else
        compose logs -f
    fi
}

cmd_infra() {
    check_docker
    setup_env

    log_info "Starting infrastructure only (Kafka, PostgreSQL, TimescaleDB)..."
    compose up -d zookeeper kafka kafka-init kafka-console postgres timescaledb

    log_success "Infrastructure started!"
    echo ""
    echo "Now you can run Fen services locally:"
    echo "  CLUSTER_MODE=distributed \\"
    echo "  KAFKA_BOOTSTRAP_SERVERS=localhost:9092 \\"
    echo "  cargo run -p fen-api"
}

cmd_build() {
    log_info "Building Fen Docker images..."
    compose build
    log_success "Build complete"
}

cmd_clean() {
    log_warn "This will remove all data volumes!"
    read -p "Are you sure? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        compose down -v
        log_success "Cleaned up all containers and volumes"
    else
        log_info "Cancelled"
    fi
}

cmd_shell() {
    service="${1:-api}"
    log_info "Opening shell in $service container..."
    compose exec "$service" /bin/sh
}

cmd_kafka_topics() {
    log_info "Listing Kafka topics..."
    compose exec kafka kafka-topics --bootstrap-server localhost:9092 --list
}

cmd_help() {
    echo "Fen Distributed Development Environment"
    echo ""
    echo "Usage: $0 <command> [args]"
    echo ""
    echo "Commands:"
    echo "  start           Start all services"
    echo "  stop            Stop all services"
    echo "  restart         Restart all services"
    echo "  status          Show service status"
    echo "  logs [service]  Tail logs (optionally for specific service)"
    echo "  infra           Start infrastructure only (Kafka, DBs)"
    echo "  build           Build Fen Docker images"
    echo "  clean           Stop and remove all volumes"
    echo "  shell [service] Open shell in container (default: api)"
    echo "  topics          List Kafka topics"
    echo "  help            Show this help"
    echo ""
    echo "Examples:"
    echo "  $0 start        # Start everything"
    echo "  $0 infra        # Start only Kafka and databases"
    echo "  $0 logs api     # Tail API logs"
    echo "  $0 shell data-node-0  # Open shell in data node"
}

# Main command dispatch
case "${1:-help}" in
    start)
        cmd_start
        ;;
    stop)
        cmd_stop
        ;;
    restart)
        cmd_restart
        ;;
    status)
        cmd_status
        ;;
    logs)
        cmd_logs "$2"
        ;;
    infra)
        cmd_infra
        ;;
    build)
        cmd_build
        ;;
    clean)
        cmd_clean
        ;;
    shell)
        cmd_shell "$2"
        ;;
    topics)
        cmd_kafka_topics
        ;;
    help|--help|-h)
        cmd_help
        ;;
    *)
        log_error "Unknown command: $1"
        cmd_help
        exit 1
        ;;
esac
