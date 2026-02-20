//! GLiNER - Zero-shot Named Entity Recognition via ONNX
//!
//! Two-tier extraction architecture:
//! - Medium model (~60ms): first pass for most documents
//! - Large model (~200ms): escalation when medium confidence is unsatisfactory
//!
//! GLiNER operates on text only, eliminating the page rendering bottleneck
//! for text-based PDFs.

pub mod confidence;
pub mod config;
pub mod extractor;
pub mod model;
pub mod postprocessor;
pub mod preprocessor;
pub mod types;

pub use confidence::ExtractionConfidence;
pub use config::{GlinerConfig, GlinerModelConfig};
pub use extractor::GlinerExtractor;
pub use model::GlinerModel;
pub use types::{GlinerEntity, GlinerExtractionResult, ModelTier};
