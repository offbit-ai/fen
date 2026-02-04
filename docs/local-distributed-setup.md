# Local Distributed Infrastructure Setup

This guide explains how to run the Fen distributed infrastructure locally for development and testing.

## Architecture Overview

The distributed setup includes:

```
┌─────────────────────────────────────────────────────────────────┐
│                      Docker Network                              │
│                                                                  │
│  ┌──────────┐    ┌──────────┐    ┌──────────────────────────┐  │
│  │ Zookeeper│───▶│  Kafka   │◀───│    Kafka Console         │  │
│  │  :2181   │    │  :9092   │    │    localhost:8082        │  │
│  └──────────┘    └──────────┘    └──────────────────────────┘  │
│                       │                                         │
│         ┌─────────────┼─────────────┐                          │
│         ▼             ▼             ▼                          │
│  ┌────────────┐ ┌───────────┐ ┌───────────┐                   │
│  │Coordinator │ │Data Node 0│ │Data Node 1│                   │
│  │  :9001/02  │ │   :9100   │ │   :9101   │                   │
│  └────────────┘ └───────────┘ └───────────┘                   │
│         │                                                       │
│         ▼                                                       │
│  ┌────────────┐                                                │
│  │  Fen API   │◀─────────── localhost:3000                    │
│  │   :3000    │                                                │
│  └────────────┘                                                │
│         │                                                       │
│  ┌──────┴──────┐                                               │
│  ▼             ▼                                               │
│  ┌──────────┐  ┌───────────┐                                   │
│  │PostgreSQL│  │TimescaleDB│                                   │
│  │  :5432   │  │   :5433   │                                   │
│  └──────────┘  └───────────┘                                   │
└─────────────────────────────────────────────────────────────────┘
```

## Prerequisites

- Docker Desktop or Docker Engine with Docker Compose
- Rust toolchain (for local development)
- 8GB+ RAM recommended

## Quick Start

### Option 1: Full Docker Stack

Start everything in Docker:

```bash
# Start the full distributed stack
make dev

# Or using the script directly
./scripts/dev-distributed.sh start
```

### Option 2: Infrastructure + Local Rust

Start infrastructure in Docker, run Rust services locally for faster iteration:

```bash
# Terminal 1: Start infrastructure (Kafka, PostgreSQL, TimescaleDB)
make infra

# Terminal 2: Run coordinator
make coordinator

# Terminal 3: Run data node (shard 0)
./scripts/run-local.sh data 0

# Terminal 4: Run data node (shard 1)
./scripts/run-local.sh data 1

# Terminal 5: Run API in distributed mode
make api-dist
```

## Access Points

| Service | URL | Credentials |
|---------|-----|-------------|
| Fen API | http://localhost:3000 | - |
| Kafka Console | http://localhost:8082 | - |
| Coordinator API | http://localhost:9002 | - |
| PostgreSQL | localhost:5432 | fen / fen_dev_password |
| TimescaleDB | localhost:5433 | fen_metrics / fen_metrics_password |

## Available Commands

### Using Make

```bash
make dev          # Start full distributed stack
make infra        # Start infrastructure only
make stop         # Stop all services
make clean        # Stop and remove volumes
make logs         # Tail all logs
make status       # Show service status

make api          # Run API standalone (local)
make api-dist     # Run API distributed (local)
make coordinator  # Run coordinator (local)
make data         # Run data node (local)

make test         # Run unit tests
make test-int     # Run integration tests
make lint         # Run linter
```

### Using Scripts

```bash
./scripts/dev-distributed.sh start    # Start everything
./scripts/dev-distributed.sh stop     # Stop everything
./scripts/dev-distributed.sh logs     # View logs
./scripts/dev-distributed.sh logs api # View specific service logs
./scripts/dev-distributed.sh shell    # Open shell in container
./scripts/dev-distributed.sh topics   # List Kafka topics
./scripts/dev-distributed.sh clean    # Remove all data

./scripts/run-local.sh api            # Run API standalone
./scripts/run-local.sh api-dist       # Run API distributed
./scripts/run-local.sh coordinator    # Run coordinator
./scripts/run-local.sh data 0         # Run data node shard 0
```

## Kafka Topics

The following topics are automatically created:

| Topic | Partitions | Retention | Purpose |
|-------|------------|-----------|---------|
| `fen.document.ingestion` | 16 | 7 days | Document upload events |
| `fen.document.processed` | 16 | 7 days | Processing completion |
| `fen.anomaly.detected` | 8 | 30 days | Anomaly alerts |
| `fen.baseline.updates` | 4 | Compacted | Baseline statistics |
| `fen.metrics` | 4 | 1 day | System metrics |
| `fen.alerts` | 4 | 30 days | User alerts |
| `fen.validation.results` | 8 | 7 days | Validation outcomes |

## Environment Configuration

Copy and customize the environment file:

```bash
cp deploy/docker/.env.distributed.example deploy/docker/.env.distributed
```

Key variables:

```bash
# Database
POSTGRES_PASSWORD=fen_dev_password
TIMESCALE_PASSWORD=fen_metrics_password

# Cluster
NUM_SHARDS=2
REPLICATION_FACTOR=1

# Logging
RUST_LOG=info,fen_api=debug
```

## Testing the Setup

### Health Checks

```bash
# API health
curl http://localhost:3000/health

# Coordinator health
curl http://localhost:9002/health

# View Kafka topics
docker exec fen-kafka kafka-topics --bootstrap-server localhost:9092 --list
```

### Upload a Document

```bash
curl -X POST http://localhost:3000/api/v1/documents \
  -F "file=@sample_invoice.pdf" \
  -F "tenant_id=test-tenant"
```

### Query Documents

```bash
curl http://localhost:3000/api/v1/documents?limit=10
```

## Troubleshooting

### Kafka Not Starting

Check Zookeeper is healthy first:

```bash
docker logs fen-zookeeper
docker exec fen-zookeeper bash -c 'echo ruok | nc localhost 2181'
```

### Data Node Not Registering

Ensure coordinator is healthy and accepting registrations:

```bash
curl http://localhost:9002/health
curl http://localhost:9002/cluster/shards
```

### Port Conflicts

Default ports used:
- 2181: Zookeeper
- 9092: Kafka
- 3000: Fen API
- 5432: PostgreSQL
- 5433: TimescaleDB
- 8082: Kafka Console
- 9001/9002: Coordinator
- 9100/9101: Data nodes

If ports conflict, edit `docker-compose.distributed.yml`.

### Memory Issues

For constrained environments, set limits in `.env.distributed`:

```bash
KAFKA_HEAP_OPTS=-Xmx512m -Xms512m
```

## Development Workflow

1. **Start infrastructure**: `make infra`
2. **Make code changes** in your IDE
3. **Run locally**: `make api` or `make api-dist`
4. **Test**: `make test`
5. **Rebuild if needed**: `make build`

For full stack testing:

1. `make dev` - Start everything in Docker
2. `make logs` - Monitor in one terminal
3. Test via API at http://localhost:3000

## Cleanup

```bash
# Stop services but keep data
make stop

# Stop and remove all volumes (full reset)
make clean
```
