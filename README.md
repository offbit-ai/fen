# Fen

Real-time invoice and contract inconsistency detection system built in Rust.

Fen automatically extracts, validates, and cross-references financial documents using ML-powered document intelligence, rule-based validation, and tiered storage with semantic search capabilities.

## Features

- **PDF Ingestion**: Extract text and structured data from invoices using pdfium-render
- **ML Document Intelligence**: LayoutLMv3-based entity extraction, table detection, and OCR
- **Rule Engine**: GoRules Zen integration with built-in structural validations
- **Tiered Storage**: Hot (redb) + Warm (LanceDB) architecture with automatic tier migration
- **Vector Search**: Semantic similarity search on document embeddings
- **Hybrid Queries**: Combined SQL filters + vector search via LanceDB

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         fen-api (REST)                          │
├─────────────────────────────────────────────────────────────────┤
│  fen-ingestion  │  fen-rules   │  fen-ml   │   fen-storage      │
│  (PDF parsing)  │  (Zen engine)│  (ONNX)   │   (tiered)         │
├─────────────────────────────────────────────────────────────────┤
│                         fen-core (domain)                       │
└─────────────────────────────────────────────────────────────────┘
```

## Project Structure

```
fen/
├── crates/
│   ├── fen-core/       # Domain types: Invoice, Contract, Party, Anomaly
│   ├── fen-storage/    # Tiered storage (redb + LanceDB), query engine, cache
│   ├── fen-ingestion/  # PDF extraction and invoice parsing
│   ├── fen-ml/         # ONNX models: LayoutLMv3, embeddings, table extraction
│   ├── fen-rules/      # GoRules Zen engine + structural validations
│   └── fen-api/        # Axum REST API
├── rules/              # JDM decision files for Zen engine
├── Dockerfile
└── docker-compose.yml
```

## Crates

| Crate | Description |
|-------|-------------|
| `fen-core` | Domain models with serde serialization |
| `fen-storage` | Hot tier (redb, <30 days), warm tier (LanceDB, vector search) |
| `fen-ingestion` | PDF text extraction, regex + ML-based invoice parsing |
| `fen-ml` | Document intelligence: layout analysis, embeddings, OCR, tables |
| `fen-rules` | Validation engine: math checks, date validation, custom rules |
| `fen-api` | REST endpoints for document upload, query, and validation |

## Storage Tiers

| Tier | Backend | Age | Use Case |
|------|---------|-----|----------|
| Hot | redb | <30 days | Sub-ms key-value lookups, ACID transactions |
| Warm | LanceDB | 30-365 days | Vector search, hybrid SQL queries |
| Cold | LanceDB | >365 days | Archived data, batch analytics |

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
| `POST` | `/query` | Execute query (SQL + vector) |
| `GET` | `/health` | Health check |

## Key Dependencies

- **tokio** - Async runtime
- **axum** - Web framework
- **redb** - Embedded ACID database (hot tier)
- **lancedb** - Vector database with SQL support (warm tier)
- **pdfium-render** - PDF text extraction
- **ort** - ONNX Runtime for ML inference
- **zen-engine** - GoRules decision engine
- **arrow** - Columnar data format

## Validations

Built-in structural validations:
- Line items sum equals subtotal
- Total = subtotal + tax - discount
- Due date >= invoice date
- Amounts within acceptable ranges
- Required fields present

Custom rules via GoRules JDM files in `rules/` directory.

## License

MIT
