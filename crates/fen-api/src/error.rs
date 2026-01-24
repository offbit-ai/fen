use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use thiserror::Error;

/// API error types
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    #[allow(dead_code)]
    Internal(String),

    #[error("Ingestion error: {0}")]
    Ingestion(#[from] fen_ingestion::IngestionError),

    #[error("Storage error: {0}")]
    Storage(#[from] fen_storage::StorageError),

    #[error("Rule error: {0}")]
    Rule(#[from] fen_rules::RuleError),
}

/// Error response body
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match &self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg.clone()),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg.clone()),
            ApiError::Internal(msg) => {
                tracing::error!(error = %msg, "Internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    msg.clone(),
                )
            }
            ApiError::Ingestion(e) => {
                tracing::error!(error = %e, "Ingestion error");
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "ingestion_error",
                    e.to_string(),
                )
            }
            ApiError::Storage(e) => {
                tracing::error!(error = %e, "Storage error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "storage_error",
                    e.to_string(),
                )
            }
            ApiError::Rule(e) => {
                tracing::error!(error = %e, "Rule error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "rule_error",
                    e.to_string(),
                )
            }
        };

        let body = Json(ErrorResponse {
            error: error_type.to_string(),
            message,
        });

        (status, body).into_response()
    }
}

impl From<axum::extract::multipart::MultipartError> for ApiError {
    fn from(err: axum::extract::multipart::MultipartError) -> Self {
        ApiError::BadRequest(format!("Multipart error: {}", err))
    }
}
