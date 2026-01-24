# Fen

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
│                              INGESTION TIER                                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐│
│  │ PDF Parser  │  │ OCR Engine  │  │ Layout Model│  │ Table Transformer       ││
│  │ (pdfium)    │→ │ (PaddleOCR) │→ │ (LayoutLMv3)│→ │ (TATR + Structure)      ││
│  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                          PROCESSING TIER (Stateless)                             │
│  ┌──────────────────────────────────────────────────────────────────────────┐  │
│  │                    Document Processing Workers                            │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │  │
│  │  │ Normalizer  │  │ Embedding   │  │ Entity      │  │ Validation  │     │  │
│  │  │ Service     │  │ Generator   │  │ Extractor   │  │ Engine      │     │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘     │  │
│  └──────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                            STORAGE TIER (Distributed)                            │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────────┐ │
│  │   Hot Storage   │  │  Warm Storage   │  │       Cold Storage              │ │
│  │   (redb)        │  │   (LanceDB)     │  │   (DataFusion + Parquet/S3)    │ │
│  │   < 30 days     │  │   30-365 days   │  │   > 365 days                   │ │
│  │   Sub-ms reads  │  │   < 10ms reads  │  │   < 100ms reads                │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         QUERY & DETECTION TIER                                   │
│  ┌────────────────────────────────────────────────────────────────────────────┐│
│  │                      Unified Query Engine                                   ││
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   ││
│  │  │ SQL Parser   │  │ Vector Search│  │ Full-Text   │  │ Query        │   ││
│  │  │ (sqlparser)  │  │ (HNSW/IVF)   │  │ (Tantivy)   │  │ Optimizer    │   ││
│  │  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘   ││
│  └────────────────────────────────────────────────────────────────────────────┘│
│  ┌────────────────────────────────────────────────────────────────────────────┐│
│  │                   Anomaly Detection Pipeline                                ││
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   ││
│  │  │ Rule Engine  │→ │ Datalog      │→ │ ML Detector │→ │ Semantic     │   ││
│  │  │ (GoRules)    │  │ (Ascent)     │  │ (ONNX)      │  │ Analyzer     │   ││
│  │  └──────────────┘  └──────────────┘  └──────────────┘  └──────────────┘   ││
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

| Crate | Description |
|-------|-------------|
| `fen-core` | Domain models (Invoice, Contract, Party, Anomaly) with serde serialization |
| `fen-storage` | Hot tier (redb), warm tier (LanceDB), query engine, cache |
| `fen-ingestion` | PDF text extraction (pdfium), regex + ML-based invoice parsing |
| `fen-ml` | Document intelligence: LayoutLMv3, embeddings, OCR, table extraction |
| `fen-rules` | GoRules Zen engine + structural validations |
| `fen-api` | REST endpoints (axum) for document upload, query, and validation |

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
3. **Layer 3 - Statistical Anomaly** (ms latency): ONNX models for amount outliers, field-level anomaly scores
4. **Layer 4 - Semantic Contradiction** (10s of ms): NLI model for invoice terms vs contract clauses

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
