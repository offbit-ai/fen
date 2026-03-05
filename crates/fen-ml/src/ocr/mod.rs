//! OCR — Optical Character Recognition via PaddleOCR v4
//!
//! Two-stage pipeline:
//! 1. **Detection** (DBNet++): locates text regions in the image
//! 2. **Recognition** (SVTR-LCNetV2 + CTC): decodes character sequences from cropped regions
//!
//! Supports multi-language recognition with per-language models and character dictionaries.
//! Detection model is shared across all languages. Configurable preprocessing (deskew,
//! denoise, contrast, binarization) via `PreprocessingConfig`.

pub mod engine;
pub mod languages;
pub mod preprocessing;
mod types;

pub use engine::{OcrEngine, OcrProvider};
pub use preprocessing::PreprocessingConfig;
pub use types::*;
