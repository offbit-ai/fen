//! Donut — Vision Encoder-Decoder for end-to-end document understanding
//!
//! Architecture: Swin Transformer encoder + BART decoder.
//! Processes document images directly to structured JSON without OCR.
//!
//! Autoregressive decoding loop:
//! 1. Encode image → hidden states
//! 2. Seed decoder with task-specific prompt token (`<s_cord-v2>`)
//! 3. Loop: previous tokens + encoder states → next token logits → greedy argmax
//! 4. Stop at EOS or max length
//! 5. Parse output as JSON
//!
//! Used as a parallel/fallback path when OCR → LayoutLMv3 produces low confidence (<0.5).

mod decoder;
mod model;

pub use model::{DonutModel, DonutModelConfig, DonutOutput};
