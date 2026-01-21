use serde::{Deserialize, Serialize};

/// Configuration for embedding model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModelConfig {
    /// Path to ONNX model
    pub model_path: Option<String>,

    /// Embedding dimension
    pub embedding_dim: usize,

    /// Maximum sequence length
    pub max_seq_length: usize,

    /// Model name (for display/logging)
    pub model_name: String,

    /// Enable GPU acceleration
    pub gpu_enabled: bool,
}

impl Default for EmbeddingModelConfig {
    fn default() -> Self {
        Self {
            model_path: None,
            embedding_dim: 768,
            max_seq_length: 512,
            model_name: "all-MiniLM-L6-v2".to_string(),
            gpu_enabled: false,
        }
    }
}

/// Document embeddings at multiple granularities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentEmbeddings {
    /// Full document embedding
    pub document: Vec<f32>,

    /// Per-section embeddings
    pub sections: Vec<SectionEmbedding>,

    /// Per-entity embeddings
    pub entities: Vec<EntityEmbedding>,
}

impl Default for DocumentEmbeddings {
    fn default() -> Self {
        Self {
            document: Vec::new(),
            sections: Vec::new(),
            entities: Vec::new(),
        }
    }
}

impl DocumentEmbeddings {
    pub fn new(embedding_dim: usize) -> Self {
        Self {
            document: vec![0.0; embedding_dim],
            sections: Vec::new(),
            entities: Vec::new(),
        }
    }
}

/// Embedding for a document section
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionEmbedding {
    /// Section identifier
    pub section_id: String,

    /// Section type
    pub section_type: String,

    /// Embedding vector
    pub embedding: Vec<f32>,

    /// Text content
    pub text: String,
}

/// Embedding for a named entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityEmbedding {
    /// Entity type
    pub entity_type: String,

    /// Entity value
    pub value: String,

    /// Embedding vector
    pub embedding: Vec<f32>,
}

/// Similarity result
#[derive(Debug, Clone)]
pub struct SimilarityResult {
    /// Index of the compared item
    pub index: usize,

    /// Similarity score (0.0 - 1.0)
    pub score: f32,

    /// Optional identifier
    pub id: Option<String>,
}

impl SimilarityResult {
    pub fn new(index: usize, score: f32) -> Self {
        Self {
            index,
            score,
            id: None,
        }
    }

    pub fn with_id(mut self, id: String) -> Self {
        self.id = Some(id);
        self
    }
}
