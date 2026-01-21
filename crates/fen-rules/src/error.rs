use thiserror::Error;

/// Rule engine errors
#[derive(Error, Debug)]
pub enum RuleError {
    #[error("Rule evaluation error: {0}")]
    Evaluation(String),

    #[error("Rule loading error: {0}")]
    Loading(String),

    #[error("Rule parsing error: {0}")]
    Parsing(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
