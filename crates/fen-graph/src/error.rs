use thiserror::Error;

#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Graph database error: {0}")]
    Database(String),

    #[error("Graph schema initialization failed: {0}")]
    SchemaInit(String),

    #[error("Graph query error: {0}")]
    Query(String),

    #[error("Graph write error: {0}")]
    Write(String),

    #[error("Entity not found: {entity_type} with id {id}")]
    NotFound { entity_type: String, id: String },

    #[error("Task join error: {0}")]
    TaskJoin(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<ryugraph::Error> for GraphError {
    fn from(e: ryugraph::Error) -> Self {
        GraphError::Database(e.to_string())
    }
}

impl From<tokio::task::JoinError> for GraphError {
    fn from(e: tokio::task::JoinError) -> Self {
        GraphError::TaskJoin(e.to_string())
    }
}
