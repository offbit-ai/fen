//! Search endpoints for full-text and semantic search

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use fen_storage::{parse_query, FullTextSearchResult, QueryParams};

use crate::error::ApiError;
use crate::state::AppState;

/// Query parameters for text search
#[derive(Deserialize)]
pub struct TextSearchParams {
    /// Search query string
    pub q: String,
    /// Maximum results to return (default: 10, max: 100)
    pub limit: Option<usize>,
}

/// Query parameters for semantic search
#[derive(Deserialize)]
pub struct SemanticSearchParams {
    /// Search query for embedding generation
    pub q: String,
    /// Maximum results to return (default: 10, max: 100)
    pub limit: Option<usize>,
}

/// Search result item
#[derive(Serialize)]
pub struct SearchResultItem {
    /// Document ID
    pub id: String,
    /// Relevance score
    pub score: f32,
}

impl From<FullTextSearchResult> for SearchResultItem {
    fn from(r: FullTextSearchResult) -> Self {
        Self {
            id: r.id,
            score: r.score,
        }
    }
}

/// Response for text search
#[derive(Serialize)]
pub struct TextSearchResponse {
    /// Search results
    pub results: Vec<SearchResultItem>,
    /// Original query
    pub query: String,
    /// Total results found
    pub total: usize,
}

/// Request body for query execution
#[derive(Deserialize)]
pub struct QueryRequest {
    /// SQL-like query string (e.g., "SELECT * FROM invoices WHERE vendor_name = 'Acme'")
    pub sql: String,
}

/// Response for query execution
#[derive(Serialize)]
pub struct QueryResponse {
    /// Query result rows
    pub rows: Vec<serde_json::Value>,
    /// Metadata about the query execution
    pub metadata: QueryMetadata,
}

#[derive(Serialize)]
pub struct QueryMetadata {
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of rows scanned
    pub rows_scanned: usize,
    /// Number of rows returned
    pub rows_returned: usize,
}

/// GET /search/text - Full-text search across documents
///
/// Search invoices using BM25-based full-text search.
pub async fn text_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TextSearchParams>,
) -> Result<Json<TextSearchResponse>, ApiError> {
    let limit = params.limit.unwrap_or(10).min(100);

    // Use the fulltext index if available
    let results = if let Some(ref index) = state.fulltext_index {
        index
            .search_invoices(&params.q, limit)
            .map_err(|e| ApiError::Internal(format!("Search failed: {}", e)))?
    } else {
        // Fallback: return empty results if no fulltext index
        Vec::new()
    };

    let total = results.len();
    let items: Vec<SearchResultItem> = results.into_iter().map(Into::into).collect();

    Ok(Json(TextSearchResponse {
        results: items,
        query: params.q,
        total,
    }))
}

/// GET /search/semantic - Semantic/vector similarity search
///
/// Search invoices using vector embeddings for semantic similarity.
/// Note: Requires documents to have embeddings stored in warm tier (LanceDB).
pub async fn semantic_search(
    State(_state): State<Arc<AppState>>,
    Query(params): Query<SemanticSearchParams>,
) -> Result<Json<TextSearchResponse>, ApiError> {
    let _limit = params.limit.unwrap_or(10).min(100);

    // TODO: Implement semantic search when embedding generation is available
    // This requires:
    // 1. Generate embedding from query text using ML model
    // 2. Search warm storage (LanceDB) with vector similarity
    // 3. Return results with similarity scores

    // For now, return empty results with a note
    Ok(Json(TextSearchResponse {
        results: Vec::new(),
        query: params.q,
        total: 0,
    }))
}

/// POST /search/query - Execute a FenQuery (SQL-like query)
///
/// Execute an SQL-like query against the document store.
/// Supports SELECT, WHERE, ORDER BY, LIMIT, and OFFSET clauses.
pub async fn execute_query(
    State(state): State<Arc<AppState>>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, ApiError> {
    // Parse the query
    let query = parse_query(&request.sql).map_err(|e| {
        ApiError::BadRequest(format!("Invalid query syntax: {}", e))
    })?;

    // Execute if we have a query executor
    if let Some(ref executor) = state.query_executor {
        let params = QueryParams::new();
        let result = executor
            .execute(&query, &params)
            .await
            .map_err(|e| ApiError::Internal(format!("Query execution failed: {}", e)))?;

        // Convert rows to JSON
        let rows: Vec<serde_json::Value> = result
            .rows
            .into_iter()
            .map(|row| {
                let obj: serde_json::Map<String, serde_json::Value> = row
                    .columns
                    .into_iter()
                    .map(|(k, v)| (k, column_value_to_json(v)))
                    .collect();
                serde_json::Value::Object(obj)
            })
            .collect();

        Ok(Json(QueryResponse {
            rows,
            metadata: QueryMetadata {
                execution_time_ms: result.metadata.execution_time_ms,
                rows_scanned: result.metadata.rows_scanned,
                rows_returned: result.metadata.rows_returned,
            },
        }))
    } else {
        Err(ApiError::Internal("Query executor not available".to_string()))
    }
}

/// Convert a ColumnValue to JSON
fn column_value_to_json(v: fen_storage::ColumnValue) -> serde_json::Value {
    use fen_storage::ColumnValue;
    match v {
        ColumnValue::String(s) => serde_json::Value::String(s),
        ColumnValue::Integer(i) => serde_json::Value::Number(i.into()),
        ColumnValue::Float(f) => {
            serde_json::Number::from_f64(f).map_or(serde_json::Value::Null, |n| {
                serde_json::Value::Number(n)
            })
        }
        ColumnValue::Decimal(d) => serde_json::Value::String(d.to_string()),
        ColumnValue::Date(d) => serde_json::Value::String(d.to_string()),
        ColumnValue::Boolean(b) => serde_json::Value::Bool(b),
        ColumnValue::Null => serde_json::Value::Null,
    }
}
