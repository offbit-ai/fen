# Fen

[![CI](https://github.com/offbit-ai/fen/actions/workflows/ci.yml/badge.svg)](https://github.com/offbit-ai/fen/actions/workflows/ci.yml)
[![Crates](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml)

Real-time invoice and contract inconsistency detection system built in Rust.

Fen automatically ingests, parses, and analyzes invoices and contracts at scale while detecting structural and semantic inconsistencies with sub-millisecond query times. The system combines modern document understanding models with a tiered storage architecture and multi-layer anomaly detection pipeline.

## Core Capabilities

- Ingest 10,000+ documents/hour with automated structure extraction
- Sub-millisecond P99 query latency on hybrid SQL + vector queries
- Real-time inconsistency detection across structural, temporal, relational, and semantic dimensions
- Horizontal scaling to petabyte-scale document archives
- Strong consistency guarantees for financial audit compliance

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         INGESTION TIER (fen-ingestion)                            │
│                                                                                  │
│  ┌─────────────┐                                                                 │
│  │ PDF Parser  │──→ text + rendered pages                                        │
│  │ (pdfium)    │                                                                 │
│  └─────────────┘       │                                                         │
│                        ├──────────────────────────────────────┐                   │
│                        ▼                                      ▼                   │
│  ┌──── Text Path (fastest) ──────┐   ┌──── Image Path (scanned docs) ──────────┐│
│  │                                │   │                                          ││
│  │  ┌───────────────────────┐    │   │  ┌───────────┐  ┌────────────────────┐  ││
│  │  │ GLiNER Zero-Shot NER  │    │   │  │ PaddleOCR │→ │ LayoutLMv3         │  ││
│  │  │ Medium → Large tier   │    │   │  │ (PP-OCRv4)│  │ (layout + entities)│  ││
│  │  └───────────────────────┘    │   │  └───────────┘  └────────────────────┘  ││
│  │                                │   │       │                │                ││
│  └────────────┬───────────────────┘   │       ▼                ▼                ││
│               │                       │  ┌─────────┐  ┌────────────────────┐   ││
│               │                       │  │  TATR   │  │ Donut (fallback)   │   ││
│               │                       │  │ (tables)│  │ image→JSON <0.5    │   ││
│               │                       │  └─────────┘  └────────────────────┘   ││
│               │                       └──────────┬──────────────────────────────┘│
│               └──────────────┬───────────────────┘                               │
│                              ▼                                                   │
│  ┌──── Entity Extraction (5-tier per field) ────────────────────────────────────┐│
│  │  LayoutLMv3 entities → KV pairs → GLiNER → Rule Engine → Regex              ││
│  └──────────────────────────────────────────────────────────────────────────────┘│
│                              │                                                   │
│  ┌──── Post-Extraction ─────┼───────────────────────────────────────────────────┐│
│  │  ┌─────────────────┐  ┌──┴──────────────┐  ┌─────────────────────────────┐  ││
│  │  │ Embedding Gen   │  │ Invoice/Contract│  │ Extraction Logger (JSONL)   │  ││
│  │  │ (MiniLM-L6-v2)  │  │ Struct Builder  │  │ → GLiNER fine-tuning data   │  ││
│  │  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘  ││
│  └──────────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                          ┌─────────────┴─────────────┐
                          ▼                           ▼
┌──────────────────────────────────┐ ┌────────────────────────────────────────────┐
│    KNOWLEDGE GRAPH (fen-graph)   │ │         STORAGE TIER (Distributed)          │
│  ┌────────────────────────────┐  │ │                                            │
│  │ KyuGraph (pure Rust)      │  │ │ ┌──────────────┐ ┌──────────────┐ ┌──────┐│
│  │ Vendor ──→ Invoice        │  │ │ │ Hot Storage  │ │ Warm Storage │ │ Cold ││
│  │ Vendor ──→ Contract       │  │ │ │ (redb)      │ │ (LanceDB)   │ │Parquet││
│  │ Invoice ──→ LineItem      │  │ │ │ < 30 days   │ │ 30-365 days │ │ >365d ││
│  │ Invoice ──→ Contract      │  │ │ │ Sub-ms      │ │ < 10ms      │ │<100ms ││
│  │ + RDF export (ext-rdf)    │  │ │ └──────────────┘ └──────────────┘ └──────┘│
│  └────────────────────────────┘  │ │ ┌───────────────────┐ ┌─────────────────┐│
└──────────────────────────────────┘ │ │ Anomaly Store     │ │ Baseline Store  ││
                          │          │ │ (redb, z-scores)  │ │ (redb, rolling) ││
                          │          │ └───────────────────┘ └─────────────────┘│
                          │          └────────────────────────────────────────────┘
                          │                           │
                          └─────────────┬─────────────┘
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         QUERY & DETECTION TIER                                   │
│  ┌────────────────────────────────────────────────────────────────────────────┐│
│  │                      Unified Query Engine                                   ││
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   ││
│  │  │ FQL Parser   │  │ Vector Search│  │ Full-Text   │  │ Query        │   ││
│  │  │ (custom)     │  │ (HNSW/IVF)   │  │ (Tantivy)   │  │ Optimizer    │   ││
│  │  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘   ││
│  └────────────────────────────────────────────────────────────────────────────┘│
│  ┌────────────────────────────────────────────────────────────────────────────┐│
│  │                   Anomaly Detection Pipeline                                ││
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   ││
│  │  │ Rule Engine  │→ │ Statistical  │→ │ Datalog      │→ │ Semantic     │   ││
│  │  │ (GoRules)    │  │ Analyzer     │  │ (Ascent)     │  │ Analyzer     │   ││
│  │  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘   ││
│  │         │                 │                                                 ││
│  │         │                 ▼                                                 ││
│  │         │    ┌─────────────────────────────────────────────────────────┐   ││
│  │         │    │              Statistical Analysis                        │   ││
│  │         │    │  ┌───────────┐  ┌───────────┐  ┌───────────┐            │   ││
│  │         │    │  │ Z-Score   │  │ Percentile│  │ Trend     │            │   ││
│  │         │    │  │ Detection │  │ Scoring   │  │ Detection │            │   ││
│  │         │    │  └───────────┘  └───────────┘  └───────────┘            │   ││
│  │         │    └─────────────────────────────────────────────────────────┘   ││
│  └─────────┼──────────────────────────────────────────────────────────────────┘│
│            ▼                                                                    │
│  ┌────────────────────────────────────────────────────────────────────────────┐│
│  │                      Pipeline Operations (FQL)                              ││
│  │  VALIDATE │ ANALYZE │ ANALYZE BASELINE │ CROSS_VALIDATE │ AGGREGATE        ││
│  └────────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              API GATEWAY                                         │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐│
│  │ REST API       │  │ gRPC Services  │  │ GraphQL       │  │ WebSocket      ││
│  │ (axum)         │  │ (tonic)        │  │ (async-graphql│  │ (real-time)    ││
│  └────────────────┘  └────────────────┘  └────────────────┘  └────────────────┘│
└─────────────────────────────────────────────────────────────────────────────────┘
```

## Design Principles

| Principle | Implementation |
|-----------|----------------|
| **Zero-Copy Data Path** | rkyv serialization, memory-mapped indices, arena allocation |
| **Shared-Nothing Scaling** | Partition by tenant/document-type, independent shard processing |
| **Tiered Consistency** | Strong for financial data (redb), eventual for analytics (LanceDB) |
| **Graceful Degradation** | Circuit breakers, fallback to cached results, async retry queues |

## Crates

| Crate | Status | Description |
|-------|--------|-------------|
| `fen-core` | [![fen-core](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-core)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | Domain models (Invoice, Contract, Party, Anomaly) with serde serialization |
| `fen-storage` | [![fen-storage](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-storage)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | Hot tier (redb), warm tier (LanceDB), query engine, cache |
| `fen-ingestion` | [![fen-ingestion](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-ingestion)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | Tiered document parsing (GLiNER → ML → Regex), Donut fallback, extraction logging |
| `fen-ml` | [![fen-ml](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-ml)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | Document intelligence: OCR, LayoutLMv3, TATR, GLiNER NER, Donut vision, embeddings |
| `fen-graph` | [![fen-graph](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-graph)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | Knowledge graph (KyuGraph) with RDF support for entity relationships |
| `fen-rules` | [![fen-rules](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-rules)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | GoRules Zen engine + structural validations |
| `fen-api` | [![fen-api](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-api)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | REST endpoints (axum) for document upload, query, and validation |
| `fen-events` | [![fen-events](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-events)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | Event streaming with Kafka support and protobuf schemas |
| `fen-notify` | [![fen-notify](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-notify)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | Real-time notification delivery (WebSocket, Email, Webhook) |
| `fen-cluster` | [![fen-cluster](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-cluster)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | Shard management, routing, and 2PC transaction coordination |
| `fen-grpc` | [![fen-grpc](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-grpc)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | Inter-node gRPC communication for distributed deployment |
| `fen-coordinator` | [![fen-coordinator](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-coordinator)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | Control plane binary for cluster coordination |
| `fen-data` | [![fen-data](https://github.com/offbit-ai/fen/actions/workflows/crates.yml/badge.svg?branch=main&job=fen-data)](https://github.com/offbit-ai/fen/actions/workflows/crates.yml) | Data node binary for shard hosting |

## Distributed Architecture

Fen supports horizontal scaling through an event-driven distributed architecture with sharded storage and multi-node coordination.

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              CONTROL PLANE                                       │
│  ┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────────┐ │
│  │   Raft Coordinator  │  │   Config Service    │  │   Metadata Store        │ │
│  │   (openraft)        │  │   (etcd sync)       │  │   (shard assignments)   │ │
│  └─────────────────────┘  └─────────────────────┘  └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                    ┌───────────────────┼───────────────────┐
                    ▼                   ▼                   ▼
┌──────────────────────────────────────────────────────────────────────────────────┐
│                           EVENT BUS (Kafka)                                       │
│  ┌────────────────┐ ┌────────────────┐ ┌────────────────┐ ┌────────────────┐    │
│  │ fen.document.  │ │ fen.document.  │ │ fen.anomaly.   │ │ fen.baseline.  │    │
│  │ ingestion      │ │ processed      │ │ detected       │ │ updates        │    │
│  └────────────────┘ └────────────────┘ └────────────────┘ └────────────────┘    │
└──────────────────────────────────────────────────────────────────────────────────┘
          │                      │                      │
          ▼                      ▼                      ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         COMPUTE PLANE (Stateless)                                │
│  ┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────────┐ │
│  │  Ingestion Workers  │  │  Validation Workers │  │   Baseline Workers      │ │
│  │  (GPU for ML)       │  │  (Rule Engine)      │  │   (Stats Computation)   │ │
│  └─────────────────────┘  └─────────────────────┘  └─────────────────────────┘ │
│  ┌─────────────────────┐  ┌─────────────────────┐                              │
│  │  Notification Svc   │  │  Metrics Aggregator │  ◄── Real-time Reporting    │
│  │  (WebSocket/Email)  │  │  (Prometheus)       │                              │
│  └─────────────────────┘  └─────────────────────┘                              │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                           DATA PLANE (Sharded)                                   │
│  ┌───────────────────────────────────────────────────────────────────────────┐ │
│  │                        Shard Manager                                       │ │
│  │   Consistent Hashing: hash(tenant_id || doc_type) % num_shards            │ │
│  │   2PC Coordinator for cross-shard transactions                             │ │
│  └───────────────────────────────────────────────────────────────────────────┘ │
│                                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │   Shard 0    │  │   Shard 1    │  │   Shard 2    │  │   Shard N    │       │
│  │  ┌────────┐  │  │  ┌────────┐  │  │  ┌────────┐  │  │  ┌────────┐  │       │
│  │  │ redb   │  │  │  │ redb   │  │  │  │ redb   │  │  │  │ redb   │  │       │
│  │  │ (hot)  │  │  │  │ (hot)  │  │  │  │ (hot)  │  │  │  │ (hot)  │  │       │
│  │  ├────────┤  │  │  ├────────┤  │  │  ├────────┤  │  │  ├────────┤  │       │
│  │  │LanceDB │  │  │  │LanceDB │  │  │  │LanceDB │  │  │  │LanceDB │  │       │
│  │  │ (warm) │  │  │  │ (warm) │  │  │  │ (warm) │  │  │  │ (warm) │  │       │
│  │  └────────┘  │  │  └────────┘  │  │  └────────┘  │  │  └────────┘  │       │
│  │  3x replicas │  │  3x replicas │  │  3x replicas │  │  3x replicas │       │
│  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘       │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### Key Distributed Features

| Component | Description |
|-----------|-------------|
| **Event Streaming** | Kafka-based event bus with protobuf schemas for all domain events |
| **Consistent Hashing** | XXH3-based partition key routing for tenant/document-type sharding |
| **Two-Phase Commit** | ACID transactions across shards with automatic retry and recovery |
| **Notification Hub** | Multi-channel delivery (WebSocket, Email, Webhook) with tenant preferences |
| **gRPC Services** | Inter-node communication with connection pooling |
| **Raft Consensus** | Leader election and cluster membership (optional) |

### Deployment Modes

Fen supports both standalone and distributed deployment:

```bash
# Standalone mode (default) - single node, no external dependencies
CLUSTER_MODE=standalone cargo run -p fen-api

# Distributed mode - requires Kafka and multiple nodes
CLUSTER_MODE=distributed \
KAFKA_BOOTSTRAP_SERVERS=localhost:9092 \
NUM_SHARDS=16 \
cargo run -p fen-api
```

### Event Topics

| Topic | Description |
|-------|-------------|
| `fen.document.ingestion` | New document uploaded for processing |
| `fen.document.processed` | Document parsing and embedding complete |
| `fen.anomaly.detected` | Anomaly found during validation |
| `fen.baseline.updates` | Vendor baseline recalculated |
| `fen.metrics` | Real-time metrics for observability |
| `fen.alerts` | System alerts and notifications |

### Notification Providers

The notification system supports pluggable delivery channels:

| Provider | Mode | Use Case |
|----------|------|----------|
| WebSocket | Immediate | Real-time UI updates |
| Email | Batched | Periodic digest reports |
| Webhook | Async | External integrations |

Configure per-tenant notification preferences:

```rust
// Set tenant to receive WebSocket and Email notifications
hub.set_tenant_providers(tenant_id, vec!["websocket", "email"]);
```

## Storage Tiers

| Tier | Backend | Age | Latency | Use Case |
|------|---------|-----|---------|----------|
| Hot | redb | <30 days | P99 < 500us | Sub-ms key-value lookups, ACID transactions |
| Warm | LanceDB | 30-365 days | P99 < 10ms | Vector search, hybrid SQL queries |
| Cold | DataFusion + Parquet | >365 days | P99 < 100ms | Archived data, batch analytics |

## Anomaly Detection Pipeline

Multi-layer detection with increasing latency/sophistication:

1. **Layer 1 - Structural Validation** (us latency): GoRules Zen for math validation, format checks, range validation
2. **Layer 2 - Relational Constraints** (us-ms latency): Ascent Datalog for invoice-contract matching, duplicate detection
3. **Layer 3 - Statistical Anomaly** (ms latency): Real-time z-score analysis against vendor baselines with trend detection
4. **Layer 4 - Semantic Contradiction** (10s of ms): NLI model for invoice terms vs contract clauses

### Statistical Anomaly Detection

The statistical analysis layer compares incoming invoices against historical vendor baselines:

- **Z-Score Analysis**: Flag invoices with amounts beyond configurable standard deviations from vendor mean
- **Percentile Scoring**: Determine where an invoice falls in the vendor's historical distribution
- **Trend Detection**: Identify increasing, decreasing, stable, or volatile spending patterns
- **Seasonal Awareness**: Optional month-of-year baseline patterns for seasonal vendors
- **Rolling Windows**: Configurable baseline windows (30, 90, 365 days)

**Example Response:**

```json
{
  "anomaly_type": "StatisticalOutlier",
  "severity": "Medium",
  "description": "Invoice amount $10000.00 is 6.2 standard deviations from vendor baseline (mean: $1050.00)",
  "statistical_score": {
    "z_score": 6.2,
    "percentile": 99.8,
    "trend": "Stable",
    "is_outlier": true,
    "baseline_mean": 1050.0,
    "baseline_stddev": 145.0,
    "sample_count": 47
  }
}
```

## Fen Query Language (FQL)

Fen includes a powerful SQL-like query language with extensions for vector similarity search, cross-table ZIP queries, and pipeline operations for validation and analysis.

### Query Capabilities

| Feature | Description |
|---------|-------------|
| **SQL-like Syntax** | SELECT, FROM, WHERE, ORDER BY, LIMIT, OFFSET |
| **Vector Search** | `VECTOR_DISTANCE(embedding, :vector)` for semantic similarity |
| **Full-text Search** | `BM25_SCORE()` and `CONTAINS()` for keyword search |
| **ZIP Queries** | Cross-table joins between invoices and contracts |
| **Pipeline Operations** | Post-query validation and analysis via `\|>` syntax |

### Example Queries

```sql
-- Basic query with filter
SELECT invoice_number, vendor_name, total_amount
FROM invoices
WHERE total_amount > 1000
ORDER BY invoice_date DESC
LIMIT 20

-- Semantic search for similar invoices
SELECT *, VECTOR_DISTANCE(embedding, :query_vector) AS similarity
FROM invoices
WHERE VECTOR_DISTANCE(embedding, :query_vector) < 0.3
ORDER BY similarity ASC

-- Cross-table analysis: find invoices exceeding contract limits
SELECT inv.invoice_number, inv.total_amount, con.title, con.total_value
FROM invoices inv
ZIP contracts con ON inv.vendor_name = con.party_name
WHERE inv.total_amount > con.total_value

-- Pipeline: query with statistical baseline analysis
SELECT * FROM invoices
WHERE vendor_name = 'Acme Corp'
|> ANALYZE BASELINE vendor_name WINDOW 90 DAYS THRESHOLD 2.0
|> VALIDATE WITH ('math_check')
```

### Pipeline Operations

| Operation | Description |
|-----------|-------------|
| `VALIDATE` | Run structural validation rules |
| `ANALYZE` | Detect anomalies with optional analyzers |
| `ANALYZE BASELINE` | Statistical outlier detection against vendor baselines |
| `CROSS_VALIDATE` | Compare invoice fields against contract fields |
| `AGGREGATE` | Group and compute aggregate metrics |

See [Query Language Reference](docs/QUERY_LANGUAGE.md) for complete documentation.

## Quick Start

### Prerequisites

- Rust 1.75+
- pdfium library (for PDF processing)

### Build

```bash
cargo build --release
```

### Run Tests

```bash
cargo test --workspace
```

### Run API Server

```bash
cargo run -p fen-api
```

### Docker

```bash
docker-compose up --build
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/documents` | Upload PDF, returns parsed invoice |
| `GET` | `/documents` | List all documents |
| `GET` | `/documents/:id` | Get specific document |
| `POST` | `/validate` | Validate documents for anomalies |
| `POST` | `/query` | Execute hybrid query (SQL + vector) |
| `GET` | `/health` | Health check |

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `BIND_ADDRESS` | `0.0.0.0:3000` | Server bind address |
| `DATABASE_PATH` | `data/fen.redb` | Path to redb database |
| `RULES_PATH` | (none) | Path to GoRules decision file |
| `MAX_UPLOAD_SIZE` | `52428800` | Max upload size in bytes (50MB) |
| `RATE_LIMIT_RPS` | `100` | Requests per second limit |
| `RATE_LIMIT_BURST` | `200` | Burst capacity |

### Statistical Analysis Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `STATISTICAL_ENABLED` | `true` | Enable statistical analysis |
| `STATISTICAL_ON_INGEST` | `false` | Run analysis during document ingestion |
| `STATISTICAL_ON_VALIDATE` | `true` | Run analysis during validation |
| `STATISTICAL_THRESHOLD` | `2.0` | Z-score threshold for outlier detection |
| `STATISTICAL_METRICS` | `total_amount` | Comma-separated metrics to analyze |
| `STATISTICAL_WINDOW_DAYS` | `90` | Rolling window for baseline computation |
| `STATISTICAL_SEASONAL` | `true` | Enable seasonal baseline awareness |

## Query Performance Targets

| Query Type | P50 Latency | P99 Latency | Throughput |
|------------|-------------|-------------|------------|
| Point lookup (hot) | 100us | 500us | 100K QPS/shard |
| Range scan (hot, 100 rows) | 500us | 2ms | 50K QPS/shard |
| Vector search (top-10) | 1ms | 5ms | 10K QPS/shard |
| Hybrid (filter + vector) | 2ms | 10ms | 5K QPS/shard |
| Full-text search | 5ms | 20ms | 5K QPS/shard |

## Key Dependencies

- **tokio** - Async runtime
- **axum** - Web framework
- **redb** - Embedded ACID database (hot tier)
- **lancedb** - Vector database with SQL support (warm tier)
- **pdfium-render** - PDF text extraction
- **ort** - ONNX Runtime for ML inference
- **zen-engine** - GoRules decision engine
- **arrow** - Columnar data format
