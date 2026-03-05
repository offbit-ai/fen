//! Search endpoints for full-text and semantic search

use std::sync::Arc;

use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use fen_storage::{parse_query, DocumentStore, FullTextSearchResult, QueryParams};

use crate::error::ApiError;
use crate::routes::documents::contract_to_response;
use crate::routes::ingest::invoice_to_full_response;
use crate::state::AppState;

/// Query parameters for text search
#[derive(Deserialize)]
pub struct TextSearchParams {
    pub q: String,
    pub limit: Option<usize>,
    #[allow(dead_code)]
    pub offset: Option<usize>,
}

/// Query parameters for semantic search
#[derive(Deserialize)]
pub struct SemanticSearchParams {
    pub q: String,
    pub limit: Option<usize>,
    #[allow(dead_code)]
    pub offset: Option<usize>,
}

/// Search result item (matches frontend SearchResult type)
#[derive(Serialize)]
pub struct SearchResultItem {
    pub id: String,
    pub document_type: String,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    pub data: serde_json::Value,
}

/// Response for search (matches frontend SearchResponse type)
#[derive(Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total: usize,
    pub query: String,
    pub took_ms: u64,
}

/// Request body for query execution
#[derive(Deserialize)]
pub struct QueryRequest {
    pub sql: String,
}

/// Response for query execution
#[derive(Serialize)]
pub struct QueryResponse {
    pub rows: Vec<serde_json::Value>,
    pub metadata: QueryMetadata,
}

#[derive(Serialize)]
pub struct QueryMetadata {
    pub execution_time_ms: u64,
    pub rows_scanned: usize,
    pub rows_returned: usize,
}

/// Enrich search results with full document data
async fn enrich_results(
    state: &AppState,
    raw_results: Vec<FullTextSearchResult>,
) -> Vec<SearchResultItem> {
    let mut enriched = Vec::with_capacity(raw_results.len());

    for r in raw_results {
        // Try to parse as UUID and look up invoice or contract
        if let Ok(uuid) = uuid::Uuid::parse_str(&r.id) {
            let invoice_id = fen_core::domain::InvoiceId(uuid);
            if let Ok(Some(invoice)) = state.storage.get_invoice(&invoice_id).await {
                let data = invoice_to_full_response(&invoice);
                enriched.push(SearchResultItem {
                    id: r.id,
                    document_type: "invoice".to_string(),
                    score: r.score,
                    highlight: None,
                    data: serde_json::to_value(data).unwrap_or_default(),
                });
                continue;
            }

            let contract_id = fen_core::domain::ContractId(uuid);
            if let Ok(Some(contract)) = state.storage.get_contract(&contract_id).await {
                let data = contract_to_response(&contract);
                enriched.push(SearchResultItem {
                    id: r.id,
                    document_type: "contract".to_string(),
                    score: r.score,
                    highlight: None,
                    data: serde_json::to_value(data).unwrap_or_default(),
                });
                continue;
            }
        }

        // Fallback: return bare result
        enriched.push(SearchResultItem {
            id: r.id,
            document_type: "unknown".to_string(),
            score: r.score,
            highlight: None,
            data: serde_json::Value::Null,
        });
    }

    enriched
}

/// GET /search/text - Full-text search across documents
pub async fn text_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<TextSearchParams>,
) -> Result<Json<SearchResponse>, ApiError> {
    let limit = params.limit.unwrap_or(10).min(100);
    let start = std::time::Instant::now();

    let results = if let Some(ref index) = state.fulltext_index {
        index
            .search_invoices(&params.q, limit)
            .map_err(|e| ApiError::Internal(format!("Search failed: {}", e)))?
    } else {
        Vec::new()
    };

    let enriched = enrich_results(&state, results).await;
    let total = enriched.len();
    let took_ms = start.elapsed().as_millis() as u64;

    Ok(Json(SearchResponse {
        results: enriched,
        total,
        query: params.q,
        took_ms,
    }))
}

/// GET /search/semantic - Semantic/vector similarity search
pub async fn semantic_search(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SemanticSearchParams>,
) -> Result<Json<SearchResponse>, ApiError> {
    let limit = params.limit.unwrap_or(10).min(100);
    let start = std::time::Instant::now();

    // Need both ML pipeline (for query embedding) and warm storage (for vector search)
    let di = state.document_intelligence.as_ref().ok_or_else(|| {
        ApiError::Internal("Semantic search requires ML pipeline (ML_ENABLED=true)".to_string())
    })?;
    let warm = state.warm_storage.as_ref().ok_or_else(|| {
        ApiError::Internal("Semantic search requires warm storage".to_string())
    })?;

    // Generate query embedding
    let query_embedding = di.embedding.embed(&params.q).map_err(|e| {
        ApiError::Internal(format!("Failed to generate query embedding: {}", e))
    })?;

    // Search warm storage by vector similarity
    let warm_guard = warm.read().await;
    let results = warm_guard
        .search_invoices_by_embedding(&query_embedding, limit)
        .await
        .map_err(|e| ApiError::Internal(format!("Vector search failed: {}", e)))?;

    // Convert to SearchResultItem
    let items: Vec<SearchResultItem> = results
        .into_iter()
        .map(|(invoice, distance)| {
            let data = invoice_to_full_response(&invoice);
            SearchResultItem {
                id: invoice.id.to_string(),
                document_type: "invoice".to_string(),
                score: 1.0 - distance, // Convert distance to similarity
                highlight: None,
                data: serde_json::to_value(data).unwrap_or_default(),
            }
        })
        .collect();

    let total = items.len();
    let took_ms = start.elapsed().as_millis() as u64;

    Ok(Json(SearchResponse {
        results: items,
        total,
        query: params.q,
        took_ms,
    }))
}

/// POST /search/query - Execute a FenQuery (SQL-like query)
pub async fn execute_query(
    State(state): State<Arc<AppState>>,
    Json(request): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, ApiError> {
    let query = parse_query(&request.sql).map_err(|e| {
        ApiError::BadRequest(format!("Invalid query syntax: {}", e))
    })?;

    if let Some(ref executor) = state.query_executor {
        let params = QueryParams::new();
        let result = executor
            .execute(&query, &params)
            .await
            .map_err(|e| ApiError::Internal(format!("Query execution failed: {}", e)))?;

        let mut rows: Vec<serde_json::Value> = result
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

        // Apply graph pipeline ops if present
        if query.uses_graph() {
            if let Some(ref graph) = state.graph_store {
                if let Some(ref pipeline) = query.pipeline {
                    apply_graph_pipeline(&mut rows, pipeline, graph.as_ref()).await?;
                }
            }
        }

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

/// Apply graph pipeline operations to query result rows
async fn apply_graph_pipeline(
    rows: &mut [serde_json::Value],
    pipeline: &[fen_storage::PipelineOp],
    graph: &dyn fen_graph::GraphStore,
) -> Result<(), ApiError> {
    use fen_storage::PipelineOp;

    for op in pipeline {
        match op {
            PipelineOp::GraphTraverse { field, depth } => {
                for row in rows.iter_mut() {
                    if let Some(obj) = row.as_object_mut() {
                        let entry_value = obj
                            .get(field)
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();

                        if entry_value.is_empty() {
                            continue;
                        }

                        // Use vendor_name as graph entry point
                        if let Ok(vendor_id) = graph.resolve_vendor(&entry_value).await {
                            if let Ok(invoice_ids) = graph.invoices_for_vendor(&vendor_id).await {
                                let ids: Vec<String> =
                                    invoice_ids.iter().map(|id| id.to_string()).collect();
                                obj.insert(
                                    "graph_related_invoices".to_string(),
                                    serde_json::json!(ids),
                                );
                                obj.insert(
                                    "graph_vendor_id".to_string(),
                                    serde_json::Value::String(vendor_id.clone()),
                                );

                                // If depth > 1, also find contracts
                                if *depth > 1 {
                                    let mut contract_ids = Vec::new();
                                    for inv_id in &invoice_ids {
                                        if let Ok(cids) = graph.related_contracts(inv_id).await {
                                            for cid in cids {
                                                let s = cid.to_string();
                                                if !contract_ids.contains(&s) {
                                                    contract_ids.push(s);
                                                }
                                            }
                                        }
                                    }
                                    obj.insert(
                                        "graph_related_contracts".to_string(),
                                        serde_json::json!(contract_ids),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            PipelineOp::GraphEnrich => {
                for row in rows.iter_mut() {
                    if let Some(obj) = row.as_object_mut() {
                        // Try to enrich via invoice ID
                        if let Some(id_str) = obj.get("id").and_then(|v| v.as_str()) {
                            if let Ok(uuid) = id_str.parse::<uuid::Uuid>() {
                                let invoice_id = fen_core::domain::InvoiceId(uuid);
                                if let Ok(contracts) = graph.related_contracts(&invoice_id).await {
                                    let cids: Vec<String> =
                                        contracts.iter().map(|c| c.to_string()).collect();
                                    obj.insert(
                                        "graph_contracts".to_string(),
                                        serde_json::json!(cids),
                                    );
                                }
                            }
                        }

                        // Try to enrich via vendor name
                        if let Some(vendor) = obj.get("vendor_name").and_then(|v| v.as_str()) {
                            if let Ok(vid) = graph.resolve_vendor(vendor).await {
                                if let Ok(invoices) = graph.invoices_for_vendor(&vid).await {
                                    obj.insert(
                                        "graph_vendor_invoice_count".to_string(),
                                        serde_json::json!(invoices.len()),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            _ => {} // Non-graph ops handled elsewhere
        }
    }

    Ok(())
}

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
