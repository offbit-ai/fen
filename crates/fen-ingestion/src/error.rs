use thiserror::Error;

/// Ingestion pipeline errors
#[derive(Error, Debug)]
pub enum IngestionError {
    #[error("PDF load error: {0}")]
    PdfLoad(String),

    #[error("Text extraction error: {0}")]
    TextExtraction(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Storage error: {0}")]
    Storage(#[from] fen_storage::StorageError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
