//! Layout Analysis — LayoutLMv3 document understanding
//!
//! Multi-modal transformer that combines text tokens, bounding boxes, and image features
//! to classify document regions (Text, Title, List, Table, Figure) and extract:
//! - Named entities (Date, Amount, InvoiceNumber, Organization, Email)
//! - Key-value pairs from form-like structures
//!
//! Falls back to rule-based heuristics (position/content classification, regex NER)
//! when no ONNX model is loaded.

mod model;
mod types;

pub use model::LayoutModel;
pub use types::*;
