//! Rules management endpoints for rule engine status and configuration

use std::sync::Arc;

use axum::{extract::State, Json};
use serde::Serialize;

use fen_rules::StatisticalAnalyzerConfig;

use crate::error::ApiError;
use crate::state::AppState;

/// Response for rule engine status
#[derive(Serialize)]
pub struct RuleStatusResponse {
    /// Whether the rule engine is available
    pub available: bool,
    /// Whether custom GoRules are loaded
    pub has_custom_rules: bool,
    /// Whether statistical analysis is enabled
    pub has_statistical_analysis: bool,
}

/// Response for rule engine configuration
#[derive(Serialize)]
pub struct RuleConfigResponse {
    /// Statistical analyzer configuration (if enabled)
    pub statistical: Option<StatisticalAnalyzerConfig>,
    /// Built-in validation rules (always active)
    pub builtin_rules: Vec<String>,
}

/// GET /rules/status - Get rule engine status
///
/// Returns the current status of the rule engine including what
/// features are enabled.
pub async fn rule_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RuleStatusResponse>, ApiError> {
    Ok(Json(RuleStatusResponse {
        available: true,
        has_custom_rules: state.rule_engine.has_custom_rules(),
        has_statistical_analysis: state.rule_engine.has_statistical_analysis(),
    }))
}

/// GET /rules/config - Get rule engine configuration
///
/// Returns the current configuration of the rule engine.
pub async fn rule_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RuleConfigResponse>, ApiError> {
    // Get statistical config if analyzer is available
    let statistical = state.rule_engine.statistical_analyzer().map(|analyzer| {
        StatisticalAnalyzerConfig {
            enabled: analyzer.is_enabled(),
            threshold: analyzer.threshold(),
            min_samples: 5, // Default, not exposed by analyzer
            metrics: analyzer.metrics().to_vec(),
            outlier_severity: fen_core::domain::Severity::Medium, // Default
        }
    });

    Ok(Json(RuleConfigResponse {
        statistical,
        builtin_rules: vec![
            "required_fields".to_string(),
            "date_validation".to_string(),
            "amount_validation".to_string(),
            "currency_validation".to_string(),
            "line_item_validation".to_string(),
            "confidence_threshold".to_string(),
        ],
    }))
}
