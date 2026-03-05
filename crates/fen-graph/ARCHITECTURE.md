# fen-graph Architecture

## Overview

`fen-graph` wraps KyuGraph (pure Rust property graph) to provide a knowledge graph layer for document entity relationships. It exposes graph operations via the `GraphStore` trait.

## Graph Schema

```
(Vendor) ──SUPPLIES──→ (Invoice) ──GOVERNED_BY──→ (Contract)
                       (Invoice) ──BILLED_TO──→   (Party)
                       (Party)   ──PARTY_TO──→    (Contract)
                       (Contract)──HAS_CLAUSE──→  (Clause)
```

Five node types, five edge types. Invoice and Contract nodes carry UUID `id` properties matching domain IDs from `fen-core`. Vendor and Party nodes use deterministic keys derived from normalized names (e.g., `vendor:acme_corp`) — this enables upsert-based deduplication without read-before-write.

## GraphStore Trait

```rust
#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn write_invoice(&self, invoice: &Invoice) -> Result<(), GraphError>;
    async fn write_contract(&self, contract: &Contract) -> Result<(), GraphError>;
    async fn link_invoice_to_contract(...) -> Result<(), GraphError>;
    async fn resolve_vendor(&self, name: &str) -> Result<String, GraphError>;
    async fn invoices_for_vendor(&self, vendor_id: &str) -> Result<Vec<InvoiceId>, GraphError>;
    async fn related_contracts(&self, invoice_id: &InvoiceId) -> Result<Vec<ContractId>, GraphError>;
    async fn query_cypher(&self, cypher: &str) -> Result<Vec<Vec<String>>, GraphError>;
    async fn load_rdf(&self, path: &str) -> Result<(), GraphError>;
    // ...
}
```

All operations use `spawn_blocking` under the hood because KyuGraph queries are CPU/IO-bound.

## Delta Fast Path (Write Performance)

All write operations use KyuGraph's `DeltaBatch` + `apply_delta` instead of individual Cypher queries. This is critical for throughput at 1000+ documents/hour.

### Why delta batches?

| Approach | Cost per invoice | WAL writes | Round-trips |
|----------|-----------------|------------|-------------|
| **Old (Cypher)** | 5+ queries (create invoice, resolve vendor, create edge, resolve party, create edge) | 5+ | 5+ |
| **New (Delta)** | 1 `apply_delta` call with all upserts | 1 | 1 |

### How it works

`delta_writer.rs` builds a `DeltaBatch` containing all node upserts and edge upserts for a document:

```rust
// Single invoice → single DeltaBatch with 5 upserts
let batch = delta_writer::build_invoice_delta(&invoice);
conn.apply_delta(batch)?;  // one WAL append, atomic

// Bulk: N invoices → single DeltaBatch, single apply_delta
let batch = delta_writer::build_invoice_batch(&invoices);
conn.apply_delta(batch)?;
```

### Vendor deduplication without reads

Old approach: `MATCH (v:Vendor) WHERE v.name = 'Acme Corp' RETURN v.id` → create if not found.

New approach: deterministic primary keys from normalized names. `vendor_primary_key("Acme Corp")` → `vendor:acme_corp`. Same key = upsert merges automatically. No read-before-write needed.

```
Invoice 1 (Acme Corp) → upsert Vendor "vendor:acme_corp"
Invoice 2 (Acme Corp) → upsert Vendor "vendor:acme_corp"  ← same key, merged
```

## Integration Points

### Ingestion (write path)
`IngestionPipeline` calls `write_invoice()` / `write_contract()` after successful parsing. For bulk imports, `write_invoice_batch()` / `write_contract_batch()` process N documents in a single `apply_delta` call. Vendor deduplication is automatic via deterministic primary keys.

### FQL Pipeline (read path)
Two pipeline operations bridge FQL queries to the graph:

```sql
-- Traverse: follow vendor relationships from query results
SELECT * FROM invoices WHERE total_amount > 5000
|> GRAPH TRAVERSE vendor_name DEPTH 2

-- Enrich: add graph-derived columns to each row
SELECT * FROM invoices WHERE vendor_name = 'Acme Corp'
|> GRAPH ENRICH
```

`GRAPH TRAVERSE` resolves the named field as a vendor entry point and appends `graph_related_invoices`, `graph_vendor_id`, and (at depth > 1) `graph_related_contracts` columns.

`GRAPH ENRICH` adds `graph_contracts` (from invoice ID) and `graph_vendor_invoice_count` (from vendor name) to each result row.

### REST API
Five endpoints expose the graph directly:

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/graph/query` | POST | Execute raw Cypher |
| `/graph/vendors/:name/network` | GET | Vendor's invoice + contract network |
| `/graph/invoices/:id/contracts` | GET | Contracts related to an invoice |
| `/graph/rdf/load` | POST | Load RDF file into graph |
| `/graph/rdf/inspect` | POST | Inspect RDF file (stats, types, prefixes) |

## KyuGraph Scaling Model

KyuGraph is **not a distributed database**. It is a single-process convergence point where parallel workers merge results through conflict-free delta batches.

### How it scales

| Mechanism | Description |
|-----------|-------------|
| **Worker pool** (`kyu-coord`) | Priority task queue + thread pool. Workers are stateless and interchangeable. |
| **Delta batches** (`kyu-delta`) | Conflict-free upserts with last-write-wins (timestamp-ordered). No locks, no OCC. |
| **Multi-tenancy** | Per-tenant databases (strong isolation) or shared database with tenant-column rewriting. |
| **S3-backed storage** | Buffer pool → NVMe write-through cache → S3. Pod death = cold cache, not data loss. |
| **Cloud WAL** | Dual-write: local WAL (fsync) + remote sink for cross-AZ durability. |
| **Arrow Flight** | gRPC query interface for remote clients (Python, Java, Go). |

### What it doesn't do

- No distributed joins — workers converge at the database, not across nodes
- No automatic sharding — partitioning is application-level (by tenant, batch, or document type)
- No leader election / Raft — timestamp LWW eliminates coordination
- No 2-phase commit — delta batches are atomic within a single node

### Scaling for fen

In fen's deployment model, KyuGraph handles the document entity graph for a single node. For horizontal scaling:

1. **Fen cluster mode**: Each `fen-api` node runs its own KyuGraph instance. Ingestion events are broadcast via `fen-events` / Kafka, and each node replays writes to keep graphs eventually consistent.

2. **Read replicas**: Arrow Flight enables read-only graph queries from separate services without embedding KyuGraph directly.

3. **S3 durability**: Enable S3-backed storage in production. Graph data survives pod restarts without full re-ingestion.

4. **Delta batches for bulk import**: Use `DeltaBatchBuilder` for large document batch ingestion (e.g., migrating historical data) to avoid OCC contention.

### Current fen configuration

```
GRAPH_STORAGE_PATH=/data/graph    # Disk-backed KyuGraph
# or omit for no graph (feature degrades gracefully)
```

Graph is optional — all fen features work without it, just without relationship traversal.

## RDF Support

KyuGraph's `ext-rdf` extension enables importing external ontologies:

```rust
// Register before connect
db.register_extension(Box::new(ext_rdf::RdfExtension::new()));

// Load Turtle/N-Triples/RDF-XML
graph.load_rdf("/path/to/ontology.ttl").await?;
```

RDF `rdf:type` triples create node tables. Predicate URIs create relationship tables. Local names are used as table/column names (e.g., `foaf:Person` → `:Person`, `foaf:knows` → `:knows`).
