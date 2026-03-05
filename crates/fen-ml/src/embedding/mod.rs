//! Document Embeddings — all-MiniLM-L6-v2 sentence transformer
//!
//! Produces 384-dimensional L2-normalized embeddings for semantic search and similarity:
//! - Full document text
//! - Individual sections (by layout region)
//! - Extracted entities
//!
//! Uses mean pooling over token embeddings with an LRU cache (1024 entries)
//! for repeated text. Supports batch inference.

mod model;
mod types;

pub use model::EmbeddingModel;
pub use types::*;
