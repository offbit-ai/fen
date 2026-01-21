use thiserror::Error;

/// Core error types
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Invalid ID: {0}")]
    InvalidId(String),

    #[error("Parse error: {0}")]
    Parse(String),
}
