# Fen Query Language Reference

Fen provides a powerful SQL-like query language with extensions for vector similarity search, full-text search, and cross-table ZIP queries. This document provides a comprehensive reference for all query capabilities.

## Table of Contents

1. [Overview](#overview)
2. [Basic Syntax](#basic-syntax)
3. [Data Types](#data-types)
4. [Tables and Columns](#tables-and-columns)
5. [Expressions](#expressions)
6. [Functions](#functions)
7. [WHERE Clause](#where-clause)
8. [ORDER BY and Pagination](#order-by-and-pagination)
9. [ZIP Queries (Cross-Table)](#zip-queries-cross-table)
10. [Parameters](#parameters)
11. [Pipeline Operations](#pipeline-operations)
12. [JSON Query Format](#json-query-format)
13. [Examples](#examples)

---

## Overview

The Fen query language is designed for querying invoices and contracts with support for:

- **Standard SQL-like syntax** - SELECT, FROM, WHERE, ORDER BY, LIMIT, OFFSET
- **Vector similarity search** - Find semantically similar documents using embeddings
- **Full-text BM25 search** - Relevance-ranked text search
- **ZIP queries** - Join invoices and contracts with relationship matching
- **Parameterized queries** - Safe parameter binding for dynamic values
- **Pipeline operations** - Post-query validation, analysis, and transformation using `|>` syntax

### Query Flow

```
SQL String ──> Parser ──> AST ──> Executor ──> Results
                           │         │
                           │         └── Morsel-parallel filter → score → sort → project
                           │
                           └── Or from JSON Query
```

### Execution Model

FQL uses **morsel-driven parallelism** (inspired by HyPer/Umbra) for CPU-bound query stages.
The candidate set is partitioned into fixed-size morsels (~2048 rows) and processed in parallel
via rayon work-stealing:

```
Candidates ──┬── Morsel 0 ─── Filter → Score ──┐
             ├── Morsel 1 ─── Filter → Score ──┤── Merge + Sort → LIMIT
             ├── Morsel 2 ─── Filter → Score ──┤
             └── Morsel N ─── Filter → Score ──┘
```

- **Parallel threshold**: Sets below 4096 candidates execute sequentially to avoid ~5-10μs thread dispatch overhead
- **NUMA-aware**: Rayon's work-stealing keeps morsels on the same core for L2/L3 cache locality
- **Thread-safe evaluation**: All filter/score/project operations are pure functions with no shared mutable state

---

## Basic Syntax

### SELECT Statement

```sql
SELECT [columns]
FROM [table] [alias]
[ZIP [mode] [table] [alias] ON [condition]]
[WHERE [conditions]]
[ORDER BY [expressions]]
[LIMIT n]
[OFFSET n]
```

### Simple Examples

```sql
-- Select all columns from invoices
SELECT * FROM invoices

-- Select specific columns
SELECT invoice_number, total_amount, vendor_name FROM invoices

-- Select with alias
SELECT inv.invoice_number AS num, inv.total_amount AS amount
FROM invoices inv

-- Select with filter
SELECT * FROM invoices
WHERE vendor_name = 'Acme Corp'

-- Select with ordering and pagination
SELECT * FROM invoices
ORDER BY invoice_date DESC
LIMIT 10 OFFSET 20
```

---

## Data Types

### Literals

| Type | Examples | Description |
|------|----------|-------------|
| String | `'text'`, `"text"` | Single or double quoted strings |
| Integer | `42`, `-100` | 64-bit signed integers |
| Float | `3.14`, `-0.5` | 64-bit floating point |
| Boolean | `true`, `false` | Boolean values |
| NULL | `NULL` | Null/missing value |
| Array | `[1, 2, 3]` | Array of values (for vectors) |

### Column Types

| Column Type | Rust Type | Description |
|-------------|-----------|-------------|
| String | `String` | Text values |
| Integer | `i64` | Integer values |
| Float | `f64` | Floating point values |
| Decimal | `Decimal` | Precise decimal (monetary) values |
| Date | `NaiveDate` | Date without timezone |
| Boolean | `bool` | Boolean values |

---

## Tables and Columns

### Invoices Table

The `invoices` table contains extracted invoice data.

| Column | Type | Description |
|--------|------|-------------|
| `id` | String (UUID) | Unique invoice identifier |
| `document_id` | String (UUID) | Source document identifier |
| `invoice_number` | String | Invoice number from document |
| `invoice_date` | Date | Invoice issue date |
| `due_date` | Date (nullable) | Payment due date |
| `po_number` | String (nullable) | Purchase order reference |
| `contract_id` | String (nullable) | Linked contract ID |
| `contract_number` | String (nullable) | Contract number reference |
| `vendor_name` | String | Vendor/supplier name |
| `vendor_tax_id` | String (nullable) | Vendor tax ID |
| `bill_to_name` | String | Bill-to party name |
| `currency` | String | Currency code (USD, EUR, etc.) |
| `subtotal` | Decimal | Subtotal before tax |
| `tax_amount` | Decimal | Tax amount |
| `discount_amount` | Decimal | Discount amount |
| `total_amount` | Decimal | Total invoice amount |
| `validation_status` | String | Validation status |
| `confidence_score` | Float | Extraction confidence (0-1) |
| `extracted_text` | String | Full extracted text |
| `embedding` | Vector | Semantic embedding (384 dims) |

### Contracts Table

The `contracts` table contains extracted contract data.

| Column | Type | Description |
|--------|------|-------------|
| `id` | String (UUID) | Unique contract identifier |
| `document_id` | String (UUID) | Source document identifier |
| `contract_number` | String (nullable) | Contract number |
| `title` | String | Contract title |
| `contract_type` | String | Type (ServiceAgreement, PO, etc.) |
| `effective_date` | Date | Contract start date |
| `expiration_date` | Date (nullable) | Contract end date |
| `execution_date` | Date (nullable) | Signing date |
| `total_value` | Decimal (nullable) | Total contract value |
| `currency` | String (nullable) | Currency code |
| `party_name` | String (nullable) | Primary party name |
| `validation_status` | String | Validation status |
| `confidence_score` | Float | Extraction confidence |
| `extracted_text` | String | Full extracted text |
| `embedding` | Vector | Semantic embedding |

---

## Expressions

### Column References

```sql
-- Simple column
SELECT invoice_number FROM invoices

-- Qualified column (with table alias)
SELECT inv.invoice_number FROM invoices inv

-- Wildcard (all columns)
SELECT * FROM invoices

-- Qualified wildcard
SELECT inv.* FROM invoices inv
```

### Arithmetic Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Addition | `subtotal + tax_amount` |
| `-` | Subtraction | `total_amount - discount_amount` |
| `*` | Multiplication | `quantity * unit_price` |
| `/` | Division | `total_amount / 100` |
| `%` | Modulo | `amount % 10` |

### Comparison Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `=` | Equal | `vendor_name = 'Acme'` |
| `!=`, `<>` | Not equal | `status != 'paid'` |
| `<` | Less than | `total_amount < 1000` |
| `<=` | Less or equal | `invoice_date <= '2024-01-01'` |
| `>` | Greater than | `confidence_score > 0.8` |
| `>=` | Greater or equal | `due_date >= '2024-06-01'` |

### Logical Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `AND` | Logical AND | `a > 1 AND b < 10` |
| `OR` | Logical OR | `status = 'paid' OR status = 'pending'` |
| `NOT` | Logical NOT | `NOT is_void` |

### String Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `LIKE` | Pattern match (case-sensitive) | `vendor_name LIKE 'Acme%'` |
| `ILIKE` | Pattern match (case-insensitive) | `vendor_name ILIKE '%corp%'` |

Pattern wildcards:
- `%` - matches any sequence of characters
- `_` - matches any single character

### NULL Handling

```sql
-- Check for NULL
WHERE due_date IS NULL

-- Check for NOT NULL
WHERE po_number IS NOT NULL
```

---

## Functions

### Vector Distance Function

`VECTOR_DISTANCE(column, vector)` computes the cosine distance between a document's embedding and a query vector. Lower values indicate higher similarity.

```sql
-- Select with vector distance
SELECT
    invoice_number,
    VECTOR_DISTANCE(embedding, :query_vector) AS distance
FROM invoices
WHERE VECTOR_DISTANCE(embedding, :query_vector) < 0.3
ORDER BY distance ASC
LIMIT 10
```

**Parameters:**
- `column` - The embedding column (usually `embedding`)
- `vector` - A parameter containing the query vector (384 floats)

**Returns:** Float between 0 (identical) and 2 (opposite)

### BM25 Score Function

`BM25_SCORE(column, search_terms)` computes the BM25 relevance score for text search. Higher values indicate better matches.

```sql
-- Full-text search with BM25 scoring
SELECT
    invoice_number,
    BM25_SCORE(extracted_text, :search_terms) AS relevance
FROM invoices
WHERE BM25_SCORE(extracted_text, :search_terms) > 0
ORDER BY relevance DESC
LIMIT 10
```

**Parameters:**
- `column` - The text column to search (usually `extracted_text`)
- `search_terms` - A parameter containing the search query string

**Returns:** Float, higher is more relevant

### Contains Function

`CONTAINS(column, search_terms)` checks if text contains the search terms using full-text indexing. Used in WHERE clause for filtering.

```sql
-- Filter using CONTAINS
SELECT * FROM invoices
WHERE CONTAINS(extracted_text, :search_terms)
```

**Parameters:**
- `column` - The text column to search
- `search_terms` - A parameter containing the search query

**Returns:** Boolean

### Aggregate Functions

| Function | Description | Example |
|----------|-------------|---------|
| `COUNT(*)` | Count rows | `COUNT(*)` |
| `COUNT(column)` | Count non-null values | `COUNT(po_number)` |
| `SUM(column)` | Sum values | `SUM(total_amount)` |
| `AVG(column)` | Average value | `AVG(confidence_score)` |
| `MIN(column)` | Minimum value | `MIN(invoice_date)` |
| `MAX(column)` | Maximum value | `MAX(total_amount)` |

### Hybrid Search (Score Fusion)

Combine vector and text search for optimal results:

```sql
SELECT
    invoice_number,
    vendor_name,
    VECTOR_DISTANCE(embedding, :query_vector) AS semantic_score,
    BM25_SCORE(extracted_text, :search_terms) AS text_score
FROM invoices
WHERE VECTOR_DISTANCE(embedding, :query_vector) < 0.5
    AND CONTAINS(extracted_text, :search_terms)
ORDER BY 0.7 * semantic_score + 0.3 * text_score ASC
LIMIT 10
```

---

## WHERE Clause

The WHERE clause filters results based on conditions.

### Basic Filtering

```sql
-- Equality
WHERE vendor_name = 'Acme Corp'

-- Comparison
WHERE total_amount > 1000

-- Range
WHERE invoice_date >= '2024-01-01' AND invoice_date <= '2024-12-31'

-- Pattern matching
WHERE vendor_name LIKE '%Corp%'

-- NULL check
WHERE po_number IS NOT NULL
```

### Compound Conditions

```sql
-- AND conditions
WHERE vendor_name = 'Acme Corp'
    AND total_amount > 1000
    AND invoice_date >= '2024-01-01'

-- OR conditions
WHERE vendor_name = 'Acme Corp'
    OR vendor_name = 'Beta Inc'

-- Mixed with parentheses
WHERE (vendor_name = 'Acme Corp' OR vendor_name = 'Beta Inc')
    AND total_amount > 1000

-- NOT
WHERE NOT (status = 'void' OR status = 'cancelled')
```

### Search Conditions

```sql
-- Full-text search
WHERE CONTAINS(extracted_text, :search_terms)

-- Vector similarity threshold
WHERE VECTOR_DISTANCE(embedding, :query_vector) < 0.3

-- Combined search
WHERE CONTAINS(extracted_text, :keywords)
    AND VECTOR_DISTANCE(embedding, :vector) < 0.5
```

---

## ORDER BY and Pagination

### ORDER BY

```sql
-- Single column ascending (default)
ORDER BY invoice_date

-- Single column descending
ORDER BY invoice_date DESC

-- Multiple columns
ORDER BY vendor_name ASC, invoice_date DESC

-- By expression
ORDER BY total_amount - discount_amount DESC

-- By function result
ORDER BY VECTOR_DISTANCE(embedding, :query_vector) ASC

-- NULLS handling
ORDER BY due_date ASC NULLS LAST
ORDER BY po_number DESC NULLS FIRST
```

### Pagination

```sql
-- Limit results
LIMIT 10

-- Skip results
OFFSET 20

-- Pagination (page 3, 10 items per page)
LIMIT 10 OFFSET 20
```

---

## ZIP Queries (Cross-Table)

ZIP queries allow joining invoices and contracts to analyze relationships and detect inconsistencies.

### ZIP Syntax

```sql
SELECT [columns]
FROM [primary_table] [alias]
[INNER|LEFT|CROSS] ZIP [secondary_table] [alias] ON [join_condition]
[WHERE [conditions]]
```

### ZIP Modes

| Mode | Description |
|------|-------------|
| `ZIP` or `INNER ZIP` | Only pairs where both sides match |
| `LEFT ZIP` | All from primary table, matching from secondary |
| `CROSS ZIP` | Cartesian product with ON filter |

### Basic ZIP Examples

```sql
-- Match invoices to contracts by vendor name
SELECT
    inv.invoice_number,
    inv.total_amount,
    con.title,
    con.total_value
FROM invoices inv
ZIP contracts con ON inv.vendor_name = con.party_name

-- LEFT ZIP - include unmatched invoices
SELECT
    inv.invoice_number,
    con.title
FROM invoices inv
LEFT ZIP contracts con ON inv.vendor_name = con.party_name

-- Match by contract ID
SELECT inv.*, con.*
FROM invoices inv
ZIP contracts con ON inv.contract_id = con.id

-- Match by contract number
SELECT inv.*, con.*
FROM invoices inv
ZIP contracts con ON inv.contract_number = con.contract_number
```

### ZIP with WHERE Clause

WHERE conditions in ZIP queries are intelligently split:

1. **Invoice-only conditions** - Applied before join (optimization)
2. **Contract-only conditions** - Applied before join (optimization)
3. **Cross-table conditions** - Applied after join

```sql
-- Mixed conditions
SELECT inv.invoice_number, con.title
FROM invoices inv
ZIP contracts con ON inv.vendor_name = con.party_name
WHERE inv.total_amount > 1000           -- Invoice-only (pre-filter)
    AND con.effective_date > '2024-01-01' -- Contract-only (pre-filter)
    AND inv.total_amount < con.total_value -- Cross-table (post-join)
```

### Cross-Table Comparisons

```sql
-- Find invoices exceeding contract value
SELECT
    inv.invoice_number,
    inv.total_amount,
    con.total_value,
    inv.total_amount - con.total_value AS overage
FROM invoices inv
ZIP contracts con ON inv.vendor_name = con.party_name
WHERE inv.total_amount > con.total_value

-- Find invoices within contract period
SELECT inv.*, con.title
FROM invoices inv
ZIP contracts con ON inv.vendor_name = con.party_name
WHERE inv.invoice_date >= con.effective_date
    AND (con.expiration_date IS NULL OR inv.invoice_date <= con.expiration_date)
```

---

## Parameters

Parameters allow safe binding of dynamic values. Parameters are prefixed with `:`.

### Parameter Types

| Type | Description | Example |
|------|-------------|---------|
| String | Text values | `:vendor_name` |
| Integer | Integer values | `:min_amount` |
| Float | Floating point | `:threshold` |
| Boolean | Boolean values | `:include_void` |
| Vector | Float array (384 dims) | `:query_vector` |

### Using Parameters (Rust)

```rust
use fen_storage::{parse_query, QueryParams};

// Parse query with parameters
let query = parse_query(
    "SELECT * FROM invoices
     WHERE vendor_name = :vendor
       AND total_amount > :min_amount"
)?;

// Bind parameters
let params = QueryParams::new()
    .with_string("vendor", "Acme Corp")
    .with_float("min_amount", 1000.0);

// Execute
let result = executor.execute(&query, &params).await?;
```

### Vector Search with Parameters

```rust
let query = parse_query(
    "SELECT invoice_number, VECTOR_DISTANCE(embedding, :vector) AS score
     FROM invoices
     WHERE VECTOR_DISTANCE(embedding, :vector) < 0.3
     ORDER BY score ASC
     LIMIT 10"
)?;

// Generate query embedding (384 dimensions, all-MiniLM-L6-v2)
let embedding: Vec<f32> = model.encode("find invoices for software services")?;

let params = QueryParams::new()
    .with_vector("vector", embedding);

let result = executor.execute(&query, &params).await?;
```

### Text Search with Parameters

```rust
let query = parse_query(
    "SELECT invoice_number, BM25_SCORE(extracted_text, :search) AS relevance
     FROM invoices
     WHERE CONTAINS(extracted_text, :search)
     ORDER BY relevance DESC
     LIMIT 10"
)?;

let params = QueryParams::new()
    .with_string("search", "maintenance agreement");

let result = executor.execute(&query, &params).await?;
```

---

## JSON Query Format

Fen also supports a JSON query format for programmatic construction and visual query builders.

### JSON Query Structure

```json
{
  "select": [...],
  "from": { "table": "invoices", "alias": "inv" },
  "zip": { ... },
  "where": { ... },
  "orderBy": [...],
  "limit": 10,
  "offset": 0,
  "pipeline": [...],
  "params": { ... }
}
```

### SELECT Items

```json
{
  "select": [
    { "type": "wildcard" },
    { "type": "tableWildcard", "table": "inv" },
    { "type": "column", "column": "invoice_number", "alias": "num" },
    { "type": "column", "table": "inv", "column": "total_amount" },
    {
      "type": "function",
      "name": "VECTOR_DISTANCE",
      "args": [
        { "type": "column", "column": "embedding" },
        { "type": "param", "name": "vector" }
      ],
      "alias": "distance"
    }
  ]
}
```

### Conditions (WHERE)

```json
{
  "where": {
    "type": "and",
    "conditions": [
      {
        "type": "compare",
        "left": { "type": "column", "column": "vendor_name" },
        "op": "eq",
        "right": { "type": "literal", "value": "Acme Corp" }
      },
      {
        "type": "compare",
        "left": { "type": "column", "column": "total_amount" },
        "op": "gt",
        "right": { "type": "literal", "value": 1000 }
      }
    ]
  }
}
```

### Condition Types

| Type | Description |
|------|-------------|
| `compare` | Binary comparison (eq, notEq, lt, ltEq, gt, gtEq, like, iLike) |
| `between` | Range check (expr BETWEEN low AND high) |
| `in` | List membership |
| `isNull` | NULL check |
| `like` | Pattern matching |
| `function` | Function-based (CONTAINS, etc.) |
| `and` | AND of conditions |
| `or` | OR of conditions |
| `not` | Negation |

### ZIP Clause

```json
{
  "zip": {
    "table": "contracts",
    "alias": "con",
    "mode": "inner",
    "on": {
      "type": "compare",
      "left": { "type": "column", "table": "inv", "column": "vendor_name" },
      "op": "eq",
      "right": { "type": "column", "table": "con", "column": "party_name" }
    }
  }
}
```

### Parameters in JSON

```json
{
  "params": {
    "vendor": { "type": "string", "value": "Acme Corp" },
    "min_amount": { "type": "float", "value": 1000.0 },
    "query_vector": { "type": "vector", "value": [0.1, 0.2, ...] }
  }
}
```

### Builder API (Rust)

```rust
use fen_storage::{JsonQuery, JsonCondition, JsonValue, JsonTableName};

let query = JsonQuery::invoices()
    .alias("inv")
    .select_column("invoice_number")
    .select_column("total_amount")
    .filter(
        JsonCondition::eq("vendor_name", JsonValue::String("Acme".into()))
            .and(JsonCondition::gt("total_amount", JsonValue::Float(1000.0)))
    )
    .zip_with(
        JsonTableName::Contracts,
        "con",
        JsonCondition::Compare {
            left: JsonExpr::Column { table: Some("inv".into()), column: "vendor_name".into() },
            op: JsonCompareOp::Eq,
            right: JsonExpr::Column { table: Some("con".into()), column: "party_name".into() },
        },
        JsonZipMode::Inner,
    )
    .order_by_desc("invoice_date")
    .limit(10)
    .validate()
    .build();
```

---

## Pipeline Operations

Pipeline operations allow post-processing of query results for validation, analysis, and transformation. Pipeline operations are available in both SQL syntax (using `|>`) and JSON format.

### SQL Pipe Syntax

Use the `|>` operator to chain pipeline operations after a query:

```sql
SELECT inv.*, con.title
FROM invoices inv
ZIP contracts con ON inv.vendor_name = con.party_name
WHERE inv.total_amount > 1000
|> VALIDATE WITH ('math_check', 'date_check')
|> CROSS_VALIDATE ON (inv.total_amount, con.total_value) TOLERANCE 0.01
|> ANALYZE
```

#### Available Operations

| Operation | Syntax | Description |
|-----------|--------|-------------|
| VALIDATE | `\|> VALIDATE [WITH (...)] [FAIL_FAST]` | Run validation rules |
| ANALYZE | `\|> ANALYZE [WITH (...)] [INCLUDE_SCORES]` | Detect anomalies |
| ANALYZE BASELINE | `\|> ANALYZE BASELINE field [WINDOW n DAYS] [THRESHOLD n]` | Statistical baseline analysis |
| CROSS_VALIDATE | `\|> CROSS_VALIDATE ON (...) [TOLERANCE n]` | Compare fields |
| AGGREGATE | `\|> AGGREGATE BY ... INTO ...` | Group and aggregate |

#### VALIDATE

Validate documents against business rules:

```sql
-- Basic validation
SELECT * FROM invoices |> VALIDATE

-- With specific rules
SELECT * FROM invoices |> VALIDATE WITH ('math_check', 'date_check')

-- Stop on first failure
SELECT * FROM invoices |> VALIDATE WITH ('math_check') FAIL_FAST
```

#### ANALYZE

Analyze documents for anomalies:

```sql
-- Basic analysis
SELECT * FROM invoices |> ANALYZE

-- With specific analyzers and scores
SELECT * FROM invoices |> ANALYZE WITH ('anomaly_detector') INCLUDE_SCORES
```

#### ANALYZE BASELINE

Perform statistical baseline analysis to detect outliers by comparing invoice values against historical vendor baselines. This operation computes z-scores, percentiles, and trend indicators.

**Syntax:**

```sql
|> ANALYZE BASELINE <group_by_field> [WINDOW <n> DAYS] [THRESHOLD <n>] [METRICS (<metric_list>)]
```

**Parameters:**

| Parameter | Default | Description |
|-----------|---------|-------------|
| `group_by_field` | (required) | Field to group baselines by (typically `vendor_name`) |
| `WINDOW n DAYS` | 90 | Rolling window in days for baseline computation |
| `THRESHOLD n` | 2.0 | Z-score threshold for outlier detection |
| `METRICS (...)` | `total_amount` | Comma-separated list of metrics to analyze |

**Examples:**

```sql
-- Basic baseline analysis grouped by vendor
SELECT * FROM invoices
|> ANALYZE BASELINE vendor_name

-- Custom window and threshold
SELECT * FROM invoices
WHERE invoice_date >= '2024-01-01'
|> ANALYZE BASELINE vendor_name WINDOW 30 DAYS THRESHOLD 3.0

-- Analyze multiple metrics
SELECT * FROM invoices
|> ANALYZE BASELINE vendor_name WINDOW 90 DAYS METRICS (total_amount, line_item_count)

-- Combined with other pipeline operations
SELECT inv.*, con.title
FROM invoices inv
ZIP contracts con ON inv.vendor_name = con.party_name
WHERE inv.total_amount > 1000
|> ANALYZE BASELINE vendor_name THRESHOLD 2.5
|> VALIDATE WITH ('math_check')
```

**Result Fields:**

When an outlier is detected, the result includes a `StatisticalOutlier` anomaly with:

| Field | Type | Description |
|-------|------|-------------|
| `z_score` | float | Number of standard deviations from mean |
| `percentile` | float | Percentile rank (0-100) |
| `trend` | string | `Increasing`, `Decreasing`, `Stable`, or `Volatile` |
| `is_outlier` | bool | Whether value exceeds threshold |
| `baseline_mean` | float | Historical mean for this vendor |
| `baseline_stddev` | float | Historical standard deviation |
| `sample_count` | int | Number of historical samples |

**Example Response:**

```json
{
  "anomaly_type": "StatisticalOutlier",
  "severity": "Medium",
  "description": "Invoice amount $10000.00 is 6.2 standard deviations from vendor baseline (mean: $1050.00)",
  "field_path": "total_amount",
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

#### CROSS_VALIDATE

Compare invoice fields against contract fields (requires ZIP query):

```sql
SELECT inv.*, con.title
FROM invoices inv
ZIP contracts con ON inv.vendor_name = con.party_name
|> CROSS_VALIDATE ON (inv.total_amount, con.total_value)

-- With tolerance for numeric comparisons
|> CROSS_VALIDATE ON (inv.total_amount, con.total_value) TOLERANCE 0.05
```

#### AGGREGATE

Group and compute aggregate metrics:

```sql
SELECT * FROM invoices
|> AGGREGATE BY vendor_name INTO SUM(total_amount) AS total, COUNT(*) AS count
```

### JSON Format

Pipeline operations can also be specified in JSON queries:

#### Validate

```json
{
  "pipeline": [
    {
      "op": "validate",
      "rules": ["math_check", "date_check"],
      "failFast": false
    }
  ]
}
```

#### Analyze

```json
{
  "pipeline": [
    {
      "op": "analyze",
      "analyzers": ["anomaly_detector", "duplicate_detector"],
      "includeScores": true
    }
  ]
}
```

#### Analyze Baseline

Statistical baseline analysis in JSON format:

```json
{
  "pipeline": [
    {
      "op": "analyzeBaseline",
      "groupBy": "vendor_name",
      "windowDays": 90,
      "threshold": 2.0,
      "metrics": ["total_amount", "line_item_count"]
    }
  ]
}
```

**Fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `groupBy` | string | (required) | Field to group baselines by |
| `windowDays` | integer | 90 | Rolling window in days |
| `threshold` | float | 2.0 | Z-score threshold for outliers |
| `metrics` | array | `["total_amount"]` | Metrics to analyze |

#### Cross-Validate

```json
{
  "pipeline": [
    {
      "op": "crossValidate",
      "fieldMapping": {
        "total_amount": "total_value",
        "vendor_name": "party_name"
      },
      "tolerance": 0.01
    }
  ]
}
```

#### Aggregate

```json
{
  "pipeline": [
    {
      "op": "aggregate",
      "groupBy": [
        { "type": "column", "column": "vendor_name" }
      ],
      "aggregations": [
        { "function": "count", "expr": { "type": "column", "column": "id" }, "alias": "invoice_count" },
        { "function": "sum", "expr": { "type": "column", "column": "total_amount" }, "alias": "total_spend" }
      ]
    }
  ]
}
```

### Transform

Transform output fields:

```json
{
  "pipeline": [
    {
      "op": "transform",
      "mappings": {
        "full_name": { "type": "column", "column": "vendor_name" },
        "amount_cents": {
          "type": "binaryOp",
          "left": { "type": "column", "column": "total_amount" },
          "op": "mul",
          "right": { "type": "literal", "value": 100 }
        }
      }
    }
  ]
}
```

### Filter (Post-Query)

Filter results after retrieval:

```json
{
  "pipeline": [
    {
      "op": "filter",
      "condition": {
        "type": "compare",
        "left": { "type": "column", "column": "validation_status" },
        "op": "eq",
        "right": { "type": "literal", "value": "Valid" }
      }
    }
  ]
}
```

### Sort (Post-Query)

Re-sort results:

```json
{
  "pipeline": [
    {
      "op": "sort",
      "orderBy": [
        { "expr": { "type": "column", "column": "total_amount" }, "direction": "desc" }
      ]
    }
  ]
}
```

### Take (Pagination)

Limit and offset results:

```json
{
  "pipeline": [
    {
      "op": "take",
      "limit": 10,
      "offset": 20
    }
  ]
}
```

---

## Examples

### Basic Queries

```sql
-- Get all invoices
SELECT * FROM invoices

-- Get invoices for a specific vendor
SELECT * FROM invoices
WHERE vendor_name = 'Acme Corp'

-- Get high-value invoices sorted by date
SELECT invoice_number, vendor_name, total_amount
FROM invoices
WHERE total_amount > 10000
ORDER BY invoice_date DESC
LIMIT 20

-- Get invoices missing PO numbers
SELECT invoice_number, vendor_name, total_amount
FROM invoices
WHERE po_number IS NULL
ORDER BY total_amount DESC
```

### Search Queries

```sql
-- Semantic search for similar invoices
SELECT
    invoice_number,
    vendor_name,
    VECTOR_DISTANCE(embedding, :query_vector) AS similarity
FROM invoices
WHERE VECTOR_DISTANCE(embedding, :query_vector) < 0.3
ORDER BY similarity ASC
LIMIT 10

-- Full-text search
SELECT
    invoice_number,
    vendor_name,
    BM25_SCORE(extracted_text, :search) AS relevance
FROM invoices
WHERE CONTAINS(extracted_text, :search)
ORDER BY relevance DESC
LIMIT 10

-- Hybrid search (semantic + keyword)
SELECT
    invoice_number,
    vendor_name,
    VECTOR_DISTANCE(embedding, :vector) AS semantic,
    BM25_SCORE(extracted_text, :keywords) AS text
FROM invoices
WHERE VECTOR_DISTANCE(embedding, :vector) < 0.5
    AND CONTAINS(extracted_text, :keywords)
ORDER BY 0.7 * semantic + 0.3 * (1 - text/10) ASC
LIMIT 10
```

### ZIP Query Examples

```sql
-- Find all invoice-contract pairs by vendor
SELECT
    inv.invoice_number,
    inv.total_amount,
    con.title,
    con.total_value
FROM invoices inv
ZIP contracts con ON inv.vendor_name = con.party_name

-- Find invoices without matching contracts
SELECT inv.*
FROM invoices inv
LEFT ZIP contracts con ON inv.vendor_name = con.party_name
WHERE con.id IS NULL

-- Find invoices exceeding contract limits
SELECT
    inv.invoice_number,
    inv.total_amount,
    con.title,
    con.total_value,
    inv.total_amount - con.total_value AS overage
FROM invoices inv
ZIP contracts con ON inv.vendor_name = con.party_name
WHERE inv.total_amount > con.total_value
ORDER BY overage DESC

-- Find invoices outside contract period
SELECT
    inv.invoice_number,
    inv.invoice_date,
    con.title,
    con.effective_date,
    con.expiration_date
FROM invoices inv
ZIP contracts con ON inv.contract_id = con.id
WHERE inv.invoice_date < con.effective_date
    OR inv.invoice_date > con.expiration_date

-- Complex cross-table analysis
SELECT
    inv.invoice_number,
    inv.vendor_name,
    inv.total_amount,
    con.title,
    con.total_value
FROM invoices inv
ZIP contracts con ON inv.vendor_name = con.party_name
WHERE inv.total_amount > 5000              -- Invoice filter (pre-join)
    AND con.contract_type = 'ServiceAgreement' -- Contract filter (pre-join)
    AND inv.total_amount > con.total_value * 0.8 -- Cross-table (post-join)
ORDER BY inv.total_amount DESC
LIMIT 50
```

### JSON Query Examples

**Simple filter:**
```json
{
  "select": [{ "type": "wildcard" }],
  "from": { "table": "invoices" },
  "where": {
    "type": "compare",
    "left": { "type": "column", "column": "vendor_name" },
    "op": "eq",
    "right": { "type": "literal", "value": "Acme Corp" }
  },
  "limit": 10
}
```

**Nested conditions:**
```json
{
  "select": [
    { "type": "column", "column": "invoice_number" },
    { "type": "column", "column": "total_amount" }
  ],
  "from": { "table": "invoices", "alias": "inv" },
  "where": {
    "type": "and",
    "conditions": [
      {
        "type": "compare",
        "left": { "type": "column", "column": "vendor_name" },
        "op": "eq",
        "right": { "type": "param", "name": "vendor" }
      },
      {
        "type": "or",
        "conditions": [
          {
            "type": "compare",
            "left": { "type": "column", "column": "total_amount" },
            "op": "gt",
            "right": { "type": "literal", "value": 10000 }
          },
          {
            "type": "isNull",
            "expr": { "type": "column", "column": "po_number" }
          }
        ]
      }
    ]
  },
  "orderBy": [
    { "expr": { "type": "column", "column": "invoice_date" }, "direction": "desc" }
  ],
  "limit": 20,
  "params": {
    "vendor": { "type": "string", "value": "Acme Corp" }
  }
}
```

**ZIP query with pipeline:**
```json
{
  "select": [
    { "type": "tableWildcard", "table": "inv" },
    { "type": "column", "table": "con", "column": "title" },
    { "type": "column", "table": "con", "column": "total_value" }
  ],
  "from": { "table": "invoices", "alias": "inv" },
  "zip": {
    "table": "contracts",
    "alias": "con",
    "mode": "inner",
    "on": {
      "type": "compare",
      "left": { "type": "column", "table": "inv", "column": "vendor_name" },
      "op": "eq",
      "right": { "type": "column", "table": "con", "column": "party_name" }
    }
  },
  "where": {
    "type": "compare",
    "left": { "type": "column", "table": "inv", "column": "total_amount" },
    "op": "gt",
    "right": { "type": "literal", "value": 1000 }
  },
  "pipeline": [
    {
      "op": "crossValidate",
      "fieldMapping": {
        "total_amount": "total_value"
      },
      "tolerance": 0.01
    },
    {
      "op": "validate",
      "rules": ["amount_within_contract"],
      "failFast": false
    }
  ],
  "limit": 100
}
```

---

## Error Handling

### Parse Errors

Parse errors include position information for debugging:

```rust
match parse_query("SELECT * FORM invoices") {
    Ok(query) => { /* ... */ }
    Err(e) => {
        eprintln!("Parse error at {}: {}", e.span, e.kind);
        // Output: Parse error at 9..13: Expected 'FROM', found 'FORM'
    }
}
```

### Query Errors

| Error | Description |
|-------|-------------|
| `InvalidSyntax` | Query syntax is invalid |
| `UnknownTable` | Table name not recognized |
| `UnknownColumn` | Column name not found |
| `TypeMismatch` | Operation on incompatible types |
| `MissingParameter` | Required parameter not provided |
| `UnsupportedFeature` | Feature not yet implemented |

---

## Best Practices

1. **Use parameters** for all user-provided values to prevent injection
2. **Use table aliases** in ZIP queries for clarity
3. **Apply filters early** - the executor optimizes WHERE clause splitting
4. **Limit results** - always use LIMIT for large tables
5. **Use appropriate search** - vector for semantic, BM25 for keyword
6. **Index your embeddings** - create vector index for faster search
7. **Prefer JSON queries** for programmatic construction
8. **Use pipelines** for validation and analysis workflows
