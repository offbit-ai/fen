//! Document Ingestion Pipeline
//!
//! Tiered parsing strategy for invoices and contracts:
//!
//! 1. **GLiNER** (text-only, fastest): zero-shot NER on extracted PDF text
//! 2. **ML Pipeline** (OCR → LayoutLMv3 → TATR → Embeddings): for scanned/image PDFs
//! 3. **Donut** (vision fallback): end-to-end image → JSON when ML confidence is low
//! 4. **Regex** (last resort): pattern matching on raw text
//!
//! Per-field fallback chain within the ML path:
//! `LayoutLMv3 entities → KV pairs → GLiNER → Rule Engine → Regex`
//!
//! Successful extractions are logged as JSONL training data for GLiNER fine-tuning.

pub mod contract;
pub mod error;
pub mod extraction_log;
pub mod pdf;
pub mod pipeline;

pub use error::IngestionError;
pub use pipeline::{ContractIngestResult, IngestionPipeline, InvoiceIngestResult};
