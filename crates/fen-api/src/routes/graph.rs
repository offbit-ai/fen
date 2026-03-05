//! Knowledge graph API routes
//!
//! Exposes graph queries, vendor network traversal, and relationship lookups
//! via the `GraphStore` trait.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::state::AppState;

// --- Request / Response types ---

#[derive(Deserialize)]
pub struct CypherRequest {
    pub query: String,
}

#[derive(Serialize)]
pub struct CypherResponse {
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
}

#[derive(Serialize)]
pub struct VendorNetworkResponse {
    pub vendor_id: String,
    pub invoice_ids: Vec<String>,
    pub related_contract_ids: Vec<String>,
}

#[derive(Serialize)]
pub struct RelatedContractsResponse {
    pub invoice_id: String,
    pub contract_ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct RdfLoadRequest {
    pub path: String,
}

#[derive(Serialize)]
pub struct RdfLoadResponse {
    pub message: String,
}

#[derive(Deserialize)]
pub struct RdfInspectRequest {
    pub procedure: String,
    pub path: String,
}

// --- Handlers ---

fn require_graph(state: &AppState) -> Result<&dyn fen_graph::GraphStore, ApiError> {
    state
        .graph_store
        .as_deref()
        .ok_or_else(|| ApiError::BadRequest("Knowledge graph not configured".to_string()))
}

/// POST /graph/query — execute a raw Cypher query
pub async fn query_cypher(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CypherRequest>,
) -> Result<Json<CypherResponse>, ApiError> {
    let graph = require_graph(&state)?;

    let rows = graph
        .query_cypher(&req.query)
        .await
        .map_err(|e| ApiError::Internal(format!("Graph query failed: {e}")))?;

    let row_count = rows.len();
    Ok(Json(CypherResponse { rows, row_count }))
}

/// GET /graph/vendors/:name/network — get vendor's invoice + contract network
pub async fn vendor_network(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<VendorNetworkResponse>, ApiError> {
    let graph = require_graph(&state)?;

    let vendor_id = graph
        .resolve_vendor(&name)
        .await
        .map_err(|e| ApiError::Internal(format!("Vendor resolve failed: {e}")))?;

    let invoice_ids = graph
        .invoices_for_vendor(&vendor_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Invoice lookup failed: {e}")))?;

    // Collect related contracts from all invoices
    let mut contract_ids = Vec::new();
    for inv_id in &invoice_ids {
        if let Ok(contracts) = graph.related_contracts(inv_id).await {
            for cid in contracts {
                let cid_str = cid.to_string();
                if !contract_ids.contains(&cid_str) {
                    contract_ids.push(cid_str);
                }
            }
        }
    }

    Ok(Json(VendorNetworkResponse {
        vendor_id,
        invoice_ids: invoice_ids.iter().map(|id| id.to_string()).collect(),
        related_contract_ids: contract_ids,
    }))
}

/// GET /graph/invoices/:id/contracts — get contracts related to an invoice
pub async fn invoice_contracts(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RelatedContractsResponse>, ApiError> {
    let graph = require_graph(&state)?;

    let uuid: uuid::Uuid = id
        .parse()
        .map_err(|_| ApiError::BadRequest("Invalid invoice ID".to_string()))?;
    let invoice_id = fen_core::domain::InvoiceId(uuid);

    let contract_ids = graph
        .related_contracts(&invoice_id)
        .await
        .map_err(|e| ApiError::Internal(format!("Contract lookup failed: {e}")))?;

    Ok(Json(RelatedContractsResponse {
        invoice_id: id,
        contract_ids: contract_ids.iter().map(|id| id.to_string()).collect(),
    }))
}

/// POST /graph/rdf/load — load an RDF file into the graph
pub async fn rdf_load(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RdfLoadRequest>,
) -> Result<Json<RdfLoadResponse>, ApiError> {
    let graph = require_graph(&state)?;

    graph
        .load_rdf(&req.path)
        .await
        .map_err(|e| ApiError::Internal(format!("RDF load failed: {e}")))?;

    Ok(Json(RdfLoadResponse {
        message: format!("RDF loaded from {}", req.path),
    }))
}

/// POST /graph/rdf/inspect — inspect an RDF file (stats, types, prefixes)
pub async fn rdf_inspect(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RdfInspectRequest>,
) -> Result<Json<CypherResponse>, ApiError> {
    let graph = require_graph(&state)?;

    let rows = graph
        .rdf_inspect(&req.procedure, &req.path)
        .await
        .map_err(|e| ApiError::Internal(format!("RDF inspect failed: {e}")))?;

    let row_count = rows.len();
    Ok(Json(CypherResponse { rows, row_count }))
}
