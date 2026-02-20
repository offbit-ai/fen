# =============================================================================
# Fen Makefile - Development Commands
# =============================================================================
# Usage: make <target>
#
# Quick Start:
#   make infra      # Start Kafka, PostgreSQL, TimescaleDB
#   make dev        # Start full distributed stack
#   make api        # Run API locally (standalone)

.PHONY: help build test test-ml clean dev infra api api-dist coordinator data logs status stop models models-force models-clean

# Default target
help:
	@echo "Fen Development Commands"
	@echo ""
	@echo "Docker-based (full stack):"
	@echo "  make dev          Start full distributed stack in Docker"
	@echo "  make infra        Start infrastructure only (Kafka, DBs)"
	@echo "  make stop         Stop all Docker services"
	@echo "  make clean        Stop and remove all volumes"
	@echo "  make logs         Tail all Docker logs"
	@echo "  make status       Show Docker service status"
	@echo ""
	@echo "Local development (Rust services locally):"
	@echo "  make api          Run API in standalone mode"
	@echo "  make api-dist     Run API in distributed mode"
	@echo "  make coordinator  Run coordinator locally"
	@echo "  make data         Run data node locally"
	@echo ""
	@echo "Build & Test:"
	@echo "  make build        Build all Rust crates"
	@echo "  make build-docker Build Docker images"
	@echo "  make test         Run unit tests"
	@echo "  make test-int     Run integration tests"
	@echo "  make test-ml      Run ML integration tests (requires models)"
	@echo "  make lint         Run clippy and format check"
	@echo ""
	@echo "ML Models:"
	@echo "  make models       Download ONNX models from HuggingFace"
	@echo "  make models-force Re-download all models"
	@echo "  make models-clean Remove models and venv"
	@echo ""
	@echo "Frontend:"
	@echo "  make web          Start frontend dev server"
	@echo "  make web-build    Build frontend for production"

# =============================================================================
# Build & Test
# =============================================================================

build:
	cargo build --workspace

build-release:
	cargo build --workspace --release

build-docker:
	./scripts/dev-distributed.sh build

test:
	cargo test --workspace

test-int:
	./scripts/run-local.sh test

test-ml:
	@echo "Running fen-ml integration tests with real ONNX models..."
	@test -d "$${FEN_TEST_MODELS_DIR:-./models}" || (echo "Models not found. Run 'make models' first." && exit 1)
	FEN_TEST_MODELS_DIR=$${FEN_TEST_MODELS_DIR:-$(CURDIR)/models} \
	cargo test -p fen-ml \
		--test ocr_tests \
		--test layout_tests \
		--test table_tests \
		--test embedding_tests \
		--test pipeline_tests \
		--test pdf_tests \
		-- --ignored

lint:
	cargo clippy --workspace -- -D warnings
	cargo fmt --all -- --check

fmt:
	cargo fmt --all

# =============================================================================
# ML Models
# =============================================================================

models:
	./scripts/setup-models.sh --models-dir ./models

models-force:
	./scripts/setup-models.sh --models-dir ./models --force

models-clean:
	rm -rf ./models ./.venv-models

# =============================================================================
# Docker-based Development
# =============================================================================

dev:
	./scripts/dev-distributed.sh start

infra:
	./scripts/dev-distributed.sh infra

stop:
	./scripts/dev-distributed.sh stop

clean:
	./scripts/dev-distributed.sh clean

logs:
	./scripts/dev-distributed.sh logs

status:
	./scripts/dev-distributed.sh status

# =============================================================================
# Local Development (run Rust locally against Docker infra)
# =============================================================================

api:
	./scripts/run-local.sh api

api-dist:
	./scripts/run-local.sh api-dist

coordinator:
	./scripts/run-local.sh coordinator

data:
	./scripts/run-local.sh data 0

# =============================================================================
# Frontend
# =============================================================================

web:
	cd web && npm run dev

web-build:
	cd web && npm run build

web-install:
	cd web && npm install

# =============================================================================
# Database
# =============================================================================

db-shell:
	docker exec -it fen-postgres psql -U fen -d fen

db-metrics-shell:
	docker exec -it fen-timescaledb psql -U fen_metrics -d fen_metrics

# =============================================================================
# Kafka
# =============================================================================

kafka-topics:
	./scripts/dev-distributed.sh topics

kafka-console:
	@echo "Open http://localhost:8082 in your browser"

# =============================================================================
# Documentation
# =============================================================================

docs:
	cargo doc --workspace --no-deps --open
