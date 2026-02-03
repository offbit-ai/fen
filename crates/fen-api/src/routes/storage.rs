//! Storage management endpoints for tier information and cache control

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use fen_storage::DocumentStore;

use crate::error::ApiError;
use crate::state::AppState;

/// Response for tier distribution
#[derive(Serialize)]
pub struct TierDistributionResponse {
    /// Documents in hot tier (redb)
    pub hot: usize,
    /// Documents in warm tier (LanceDB)
    pub warm: usize,
    /// Documents in cold tier (archive)
    pub cold: usize,
    /// Total documents across all tiers
    pub total: usize,
}

/// Response for cache statistics
#[derive(Serialize)]
pub struct CacheStatsResponse {
    /// Number of cache hits
    pub hits: usize,
    /// Number of cache misses
    pub misses: usize,
    /// Cache hit rate (0.0 - 1.0)
    pub hit_rate: f64,
}

/// Response for migration trigger
#[derive(Serialize)]
pub struct MigrationResponse {
    /// Number of documents migrated
    pub migrated: usize,
    /// Migration status message
    pub message: String,
}

/// GET /storage/tiers - Get document tier distribution
///
/// Returns the count of documents in each storage tier.
pub async fn tier_distribution(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TierDistributionResponse>, ApiError> {
    // Get tier distribution from location index if available
    if let Some(ref location_index) = state.location_index {
        let dist = location_index.invoice_tier_distribution();
        Ok(Json(TierDistributionResponse {
            hot: dist.hot,
            warm: dist.warm,
            cold: dist.cold,
            total: dist.total(),
        }))
    } else {
        // Fallback: all documents in hot tier
        let total = state.storage.count_invoices().await?;
        Ok(Json(TierDistributionResponse {
            hot: total,
            warm: 0,
            cold: 0,
            total,
        }))
    }
}

/// GET /storage/cache/stats - Get query cache statistics
///
/// Returns cache hit/miss metrics for performance monitoring.
pub async fn cache_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CacheStatsResponse>, ApiError> {
    if let Some(ref query_engine) = state.query_engine {
        let (hits, misses) = query_engine.cache_stats();
        let total = hits + misses;
        let hit_rate = if total > 0 {
            hits as f64 / total as f64
        } else {
            0.0
        };

        Ok(Json(CacheStatsResponse {
            hits,
            misses,
            hit_rate,
        }))
    } else {
        Ok(Json(CacheStatsResponse {
            hits: 0,
            misses: 0,
            hit_rate: 0.0,
        }))
    }
}

/// POST /storage/cache/clear - Clear the query cache
///
/// Invalidates all cached query results.
pub async fn clear_cache(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ApiError> {
    if let Some(ref query_engine) = state.query_engine {
        query_engine.clear_cache();
        Ok((StatusCode::OK, Json(serde_json::json!({"message": "Cache cleared"}))))
    } else {
        Ok((StatusCode::OK, Json(serde_json::json!({"message": "No cache to clear"}))))
    }
}

/// POST /storage/migrate - Trigger tier migration
///
/// Moves eligible documents from hot to warm storage based on age.
/// This is typically done automatically, but can be triggered manually.
pub async fn trigger_migration(
    State(state): State<Arc<AppState>>,
) -> Result<Json<MigrationResponse>, ApiError> {
    if let Some(ref tiered_storage) = state.tiered_storage {
        let migrated = tiered_storage
            .migrate_to_warm()
            .await
            .map_err(|e| ApiError::Internal(format!("Migration failed: {}", e)))?;

        Ok(Json(MigrationResponse {
            migrated,
            message: format!("Successfully migrated {} documents to warm storage", migrated),
        }))
    } else {
        Ok(Json(MigrationResponse {
            migrated: 0,
            message: "Tiered storage not enabled".to_string(),
        }))
    }
}
