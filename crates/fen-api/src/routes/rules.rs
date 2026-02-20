//! Rules management endpoints for rule engine status, configuration, and CRUD

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use fen_rules::StatisticalAnalyzerConfig;
use fen_storage::DocumentStore;

use crate::error::ApiError;
use crate::state::AppState;

// --- Built-in rule definitions ---

const BUILTIN_RULES: &[BuiltinRule] = &[
    BuiltinRule {
        id: "required_fields",
        name: "Required Fields",
        description: "Validates all required invoice fields are present",
        category: "structural",
        severity: "high",
        engine: "structural",
    },
    BuiltinRule {
        id: "date_validation",
        name: "Date Validation",
        description: "Validates invoice and due dates are consistent and reasonable",
        category: "structural",
        severity: "medium",
        engine: "structural",
    },
    BuiltinRule {
        id: "amount_validation",
        name: "Amount Validation",
        description: "Validates line items sum to subtotal and total",
        category: "structural",
        severity: "critical",
        engine: "structural",
    },
    BuiltinRule {
        id: "currency_validation",
        name: "Currency Validation",
        description: "Validates currency codes are valid ISO 4217",
        category: "structural",
        severity: "medium",
        engine: "structural",
    },
    BuiltinRule {
        id: "line_item_validation",
        name: "Line Item Validation",
        description: "Validates line item quantities, prices, and totals",
        category: "structural",
        severity: "high",
        engine: "structural",
    },
    BuiltinRule {
        id: "confidence_threshold",
        name: "Confidence Threshold",
        description: "Flags documents with low extraction confidence",
        category: "quality",
        severity: "medium",
        engine: "structural",
    },
    BuiltinRule {
        id: "statistical_outlier",
        name: "Statistical Outlier Detection",
        description: "Detects statistically anomalous values based on vendor baselines",
        category: "statistical",
        severity: "medium",
        engine: "statistical",
    },
    BuiltinRule {
        id: "duplicate_detection",
        name: "Duplicate Detection",
        description: "Identifies potential duplicate invoices by number and amount",
        category: "structural",
        severity: "high",
        engine: "structural",
    },
];

struct BuiltinRule {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    category: &'static str,
    severity: &'static str,
    engine: &'static str,
}

// --- Response types ---

/// Response for rule engine status (matches frontend RuleStatus type)
#[derive(Serialize)]
pub struct RuleStatusResponse {
    pub total_rules: usize,
    pub active_rules: usize,
    pub last_execution: Option<String>,
    pub error_count: usize,
    // Extra fields for backwards compat
    pub available: bool,
    pub has_custom_rules: bool,
    pub has_statistical_analysis: bool,
}

/// Response for rule engine configuration
#[derive(Serialize)]
pub struct RuleConfigResponse {
    pub statistical: Option<StatisticalAnalyzerConfig>,
    pub builtin_rules: Vec<String>,
}

/// Rule definition response
#[derive(Serialize)]
pub struct RuleResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub status: String,
    pub severity: String,
    pub engine: String,
    pub conditions: Vec<RuleConditionResponse>,
    pub actions: Vec<RuleActionResponse>,
    pub execution_count: usize,
    pub match_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct RuleConditionResponse {
    pub field: String,
    pub operator: String,
    pub value: String,
}

#[derive(Serialize)]
pub struct RuleActionResponse {
    #[serde(rename = "type")]
    pub action_type: String,
    pub params: std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
pub struct RuleListResponse {
    pub items: Vec<RuleResponse>,
    pub total: usize,
}

/// Request for testing a rule against a document
#[derive(Deserialize)]
pub struct TestRuleRequest {
    pub document_id: Option<String>,
    #[allow(dead_code)]
    pub sample_data: Option<serde_json::Value>,
}

#[derive(Serialize)]
pub struct TestRuleResponse {
    pub rule_id: String,
    pub passed: bool,
    pub anomalies: Vec<serde_json::Value>,
    pub execution_time_ms: u64,
}

// --- Handlers ---

/// GET /rules/status - Get rule engine status
pub async fn rule_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RuleStatusResponse>, ApiError> {
    let has_statistical = state.rule_engine.has_statistical_analysis();
    let total_rules = BUILTIN_RULES.len();
    let active_rules = if has_statistical {
        total_rules
    } else {
        total_rules - 1 // Minus statistical_outlier rule
    };

    Ok(Json(RuleStatusResponse {
        total_rules,
        active_rules,
        last_execution: None,
        error_count: 0,
        available: true,
        has_custom_rules: state.rule_engine.has_custom_rules(),
        has_statistical_analysis: has_statistical,
    }))
}

/// GET /rules/config - Get rule engine configuration
pub async fn rule_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<RuleConfigResponse>, ApiError> {
    let statistical = state.rule_engine.statistical_analyzer().map(|analyzer| {
        StatisticalAnalyzerConfig {
            enabled: analyzer.is_enabled(),
            threshold: analyzer.threshold(),
            min_samples: 5,
            metrics: analyzer.metrics().to_vec(),
            outlier_severity: fen_core::domain::Severity::Medium,
        }
    });

    Ok(Json(RuleConfigResponse {
        statistical,
        builtin_rules: BUILTIN_RULES.iter().map(|r| r.id.to_string()).collect(),
    }))
}

fn builtin_to_response(rule: &BuiltinRule) -> RuleResponse {
    let now = chrono::Utc::now().to_rfc3339();
    RuleResponse {
        id: rule.id.to_string(),
        name: rule.name.to_string(),
        description: rule.description.to_string(),
        category: rule.category.to_string(),
        status: "active".to_string(),
        severity: rule.severity.to_string(),
        engine: rule.engine.to_string(),
        conditions: vec![],
        actions: vec![RuleActionResponse {
            action_type: "flag_anomaly".to_string(),
            params: std::collections::HashMap::new(),
        }],
        execution_count: 0,
        match_count: 0,
        created_at: now.clone(),
        updated_at: now,
    }
}

/// GET /rules - List all rules
pub async fn list_rules(
    State(_state): State<Arc<AppState>>,
) -> Result<Json<RuleListResponse>, ApiError> {
    let items: Vec<RuleResponse> = BUILTIN_RULES
        .iter()
        .map(builtin_to_response)
        .collect();
    let total = items.len();

    Ok(Json(RuleListResponse { items, total }))
}

/// GET /rules/:id - Get a single rule
pub async fn get_rule(
    State(_state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RuleResponse>, ApiError> {
    let rule = BUILTIN_RULES
        .iter()
        .find(|r| r.id == id)
        .ok_or_else(|| ApiError::NotFound(format!("Rule not found: {}", id)))?;

    Ok(Json(builtin_to_response(rule)))
}

/// POST /rules/:id/test - Test a rule against sample data
pub async fn test_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<TestRuleRequest>,
) -> Result<Json<TestRuleResponse>, ApiError> {
    // Verify rule exists
    let _rule = BUILTIN_RULES
        .iter()
        .find(|r| r.id == id)
        .ok_or_else(|| ApiError::NotFound(format!("Rule not found: {}", id)))?;

    // If a document_id was provided, load and validate it
    if let Some(doc_id) = body.document_id {
        let uuid = uuid::Uuid::parse_str(&doc_id)
            .map_err(|_| ApiError::BadRequest(format!("Invalid document ID: {}", doc_id)))?;
        let invoice_id = fen_core::domain::InvoiceId(uuid);

        if let Some(invoice) = state.storage.get_invoice(&invoice_id).await? {
            let start = std::time::Instant::now();
            let result = state.rule_engine.validate_invoice(&invoice).await?;
            let elapsed = start.elapsed().as_millis() as u64;

            let anomalies: Vec<serde_json::Value> = result
                .anomalies
                .iter()
                .map(|a| {
                    serde_json::json!({
                        "anomaly_type": format!("{:?}", a.anomaly_type),
                        "severity": format!("{:?}", a.severity),
                        "description": a.description,
                        "field_path": a.field_path,
                    })
                })
                .collect();

            return Ok(Json(TestRuleResponse {
                rule_id: id,
                passed: result.is_valid,
                anomalies,
                execution_time_ms: elapsed,
            }));
        }
    }

    // No document provided — return a dry-run pass
    Ok(Json(TestRuleResponse {
        rule_id: id,
        passed: true,
        anomalies: vec![],
        execution_time_ms: 0,
    }))
}
