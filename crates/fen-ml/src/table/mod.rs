//! Table Extraction — DETR-based Table Transformer (TATR)
//!
//! Two-stage pipeline:
//! 1. **Detection**: locates table bounding boxes in the document image
//! 2. **Structure Recognition**: identifies rows, columns, and spanning cells
//!
//! Cells are assigned OCR text via R-tree spatial matching.
//! Supports export to markdown, CSV, and HTML formats.

mod extractor;
mod types;

pub use extractor::TableExtractor;
pub use types::*;
