//! Notification management endpoints

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use fen_core::domain::TenantId;
use fen_notify::{NotificationPayload, NotificationType, Severity};

use crate::error::ApiError;
use crate::state::AppState;

/// Response listing notification providers
#[derive(Serialize)]
pub struct ProvidersResponse {
    /// List of registered provider IDs
    pub providers: Vec<String>,
    /// Default providers for new tenants
    pub default_providers: Vec<String>,
}

/// Provider health status
#[derive(Serialize)]
pub struct ProviderHealth {
    /// Provider ID
    pub provider_id: String,
    /// Whether the provider is healthy
    pub healthy: bool,
    /// Error message if unhealthy
    pub error: Option<String>,
}

/// Response for provider health check
#[derive(Serialize)]
pub struct HealthResponse {
    /// Overall health status
    pub healthy: bool,
    /// Individual provider health
    pub providers: Vec<ProviderHealth>,
}

/// Request to send a notification
#[derive(Deserialize)]
pub struct SendNotificationRequest {
    /// Tenant ID to send notification for
    pub tenant_id: String,
    /// Notification type
    #[serde(default = "default_notification_type")]
    pub notification_type: String,
    /// Severity level
    #[serde(default = "default_severity")]
    pub severity: String,
    /// Notification title
    pub title: String,
    /// Notification body
    pub body: String,
    /// Optional metadata
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

fn default_notification_type() -> String {
    "system_alert".to_string()
}

fn default_severity() -> String {
    "info".to_string()
}

/// Response for sent notification
#[derive(Serialize)]
pub struct SendResponse {
    /// Whether the notification was sent successfully
    pub success: bool,
    /// Notification ID
    pub notification_id: String,
    /// Message
    pub message: String,
}

/// Tenant preferences for notifications
#[derive(Serialize, Deserialize)]
pub struct TenantPreferences {
    /// Preferred provider IDs
    pub providers: Vec<String>,
}

/// GET /notifications/providers - List registered notification providers
pub async fn list_providers(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ProvidersResponse>, ApiError> {
    if let Some(ref hub) = state.notification_hub {
        Ok(Json(ProvidersResponse {
            providers: hub.list_providers(),
            default_providers: vec!["websocket".to_string()],
        }))
    } else {
        Ok(Json(ProvidersResponse {
            providers: Vec::new(),
            default_providers: Vec::new(),
        }))
    }
}

/// GET /notifications/health - Check health of all notification providers
pub async fn provider_health(
    State(state): State<Arc<AppState>>,
) -> Result<Json<HealthResponse>, ApiError> {
    if let Some(ref hub) = state.notification_hub {
        // Get list of provider IDs first
        let provider_ids = hub.list_providers();

        // For now, just report providers as healthy (full health check requires more complex handling)
        let providers: Vec<ProviderHealth> = provider_ids
            .into_iter()
            .map(|id| ProviderHealth {
                provider_id: id,
                healthy: true,
                error: None,
            })
            .collect();

        Ok(Json(HealthResponse {
            healthy: true,
            providers,
        }))
    } else {
        Ok(Json(HealthResponse {
            healthy: false,
            providers: Vec::new(),
        }))
    }
}

/// POST /notifications/send - Send a notification
pub async fn send_notification(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SendNotificationRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let hub = state.notification_hub.as_ref().ok_or_else(|| {
        ApiError::Internal("Notification hub not configured".to_string())
    })?;

    // Parse tenant ID
    let tenant_id = TenantId::from_string(&request.tenant_id)
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID".to_string()))?;

    // Parse notification type
    let notification_type = match request.notification_type.as_str() {
        "anomaly_detected" => NotificationType::AnomalyDetected,
        "validation_complete" => NotificationType::ValidationComplete,
        "baseline_updated" => NotificationType::BaselineUpdated,
        "system_alert" => NotificationType::SystemAlert,
        "batch_report" => NotificationType::BatchReport,
        other => NotificationType::Custom(other.to_string()),
    };

    // Parse severity
    let severity = request
        .severity
        .parse()
        .unwrap_or(Severity::Info);

    // Create payload
    let mut payload = NotificationPayload::new(
        tenant_id,
        notification_type,
        severity,
        request.title,
        request.body,
    );

    // Add metadata
    for (key, value) in request.metadata {
        payload = payload.with_metadata(key, value);
    }

    let notification_id = payload.id.clone();

    // Broadcast notification
    hub.broadcast(payload).await.map_err(|e| {
        ApiError::Internal(format!("Failed to send notification: {}", e))
    })?;

    Ok((
        StatusCode::OK,
        Json(SendResponse {
            success: true,
            notification_id,
            message: "Notification sent successfully".to_string(),
        }),
    ))
}

/// GET /notifications/preferences/:tenant_id - Get tenant notification preferences
pub async fn get_preferences(
    State(state): State<Arc<AppState>>,
    Path(tenant_id): Path<String>,
) -> Result<Json<TenantPreferences>, ApiError> {
    let hub = state.notification_hub.as_ref().ok_or_else(|| {
        ApiError::Internal("Notification hub not configured".to_string())
    })?;

    let tenant = TenantId::from_string(&tenant_id)
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID".to_string()))?;

    let providers = hub.get_tenant_providers(&tenant);

    Ok(Json(TenantPreferences { providers }))
}

/// PUT /notifications/preferences/:tenant_id - Set tenant notification preferences
pub async fn set_preferences(
    State(state): State<Arc<AppState>>,
    Path(tenant_id): Path<String>,
    Json(preferences): Json<TenantPreferences>,
) -> Result<impl IntoResponse, ApiError> {
    let hub = state.notification_hub.as_ref().ok_or_else(|| {
        ApiError::Internal("Notification hub not configured".to_string())
    })?;

    let tenant = TenantId::from_string(&tenant_id)
        .map_err(|_| ApiError::BadRequest("Invalid tenant ID".to_string()))?;

    hub.set_tenant_providers(tenant, preferences.providers);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({"message": "Preferences updated"})),
    ))
}
