use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use ndarray::{Array1, Array2, Array3};
use ort::session::Session;
use ort::value::TensorRef;
use tokenizers::Tokenizer;

use super::config::GlinerModelConfig;
use super::postprocessor;
use super::preprocessor::{self, GlinerInput};
use super::types::{GlinerEntity, ModelTier};
use crate::error::MlError;

/// Single GLiNER ONNX model wrapper
pub struct GlinerModel {
    config: GlinerModelConfig,
    session: Option<Mutex<Session>>,
    tokenizer: Option<Tokenizer>,
    tier: ModelTier,
}

impl GlinerModel {
    /// Create model without ONNX session (returns empty results)
    pub fn new(config: GlinerModelConfig, tier: ModelTier) -> Result<Self, MlError> {
        Ok(Self {
            config,
            session: None,
            tokenizer: None,
            tier,
        })
    }

    /// Create model with ONNX session and tokenizer
    pub fn with_model(
        config: GlinerModelConfig,
        model_path: impl AsRef<Path>,
        tokenizer_path: impl AsRef<Path>,
        tier: ModelTier,
    ) -> Result<Self, MlError> {
        let model_path = model_path.as_ref();
        let tokenizer_path = tokenizer_path.as_ref();

        if !model_path.exists() {
            return Err(MlError::ModelNotFound(model_path.display().to_string()));
        }
        if !tokenizer_path.exists() {
            return Err(MlError::ModelNotFound(tokenizer_path.display().to_string()));
        }

        let session = Session::builder()?.commit_from_file(model_path)?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| MlError::Tokenization(e.to_string()))?;

        tracing::info!(
            model = %model_path.display(),
            tokenizer = %tokenizer_path.display(),
            tier = %tier,
            "Loaded GLiNER model"
        );

        Ok(Self {
            config,
            session: Some(Mutex::new(session)),
            tokenizer: Some(tokenizer),
            tier,
        })
    }

    /// Check if ONNX model is loaded
    pub fn has_model(&self) -> bool {
        self.session.is_some() && self.tokenizer.is_some()
    }

    /// Get the model tier
    pub fn tier(&self) -> ModelTier {
        self.tier
    }

    /// Extract entities from text using this model
    ///
    /// Returns empty entities if no model is loaded.
    pub fn extract(
        &self,
        text: &str,
        labels: &[&str],
        nms_threshold: f32,
    ) -> Result<(Vec<GlinerEntity>, u64), MlError> {
        let start = Instant::now();

        if !self.has_model() {
            return Ok((Vec::new(), 0));
        }

        let tokenizer = self.tokenizer.as_ref().unwrap();

        // Preprocess
        let input = preprocessor::preprocess(
            text,
            labels,
            tokenizer,
            self.config.max_seq_length,
            self.config.max_span_width,
        )?;

        if input.span_idx.is_empty() {
            return Ok((Vec::new(), start.elapsed().as_millis() as u64));
        }

        // Run ONNX inference
        let logits = self.run_inference(&input)?;

        // Postprocess
        let num_spans = input.span_idx.len();
        let num_labels = labels.len();
        let entities = postprocessor::postprocess(
            &logits,
            num_spans,
            num_labels,
            self.config.threshold,
            nms_threshold,
            labels,
            &input,
        );

        let elapsed = start.elapsed().as_millis() as u64;

        tracing::debug!(
            tier = %self.tier,
            entities_found = entities.len(),
            elapsed_ms = elapsed,
            "GLiNER extraction complete"
        );

        Ok((entities, elapsed))
    }

    /// Run ONNX inference and return raw logits
    fn run_inference(&self, input: &GlinerInput) -> Result<Vec<f32>, MlError> {
        let mut session = self
            .session
            .as_ref()
            .ok_or_else(|| MlError::ModelLoading("GLiNER model not loaded".to_string()))?
            .lock()
            .map_err(|e| MlError::ModelLoading(format!("Failed to acquire session lock: {}", e)))?;

        let seq_len = self.config.max_seq_length;
        let num_spans = input.span_idx.len();
        let batch = 1;

        // Build tensors
        let input_ids = Array2::from_shape_vec((batch, seq_len), input.input_ids.clone())
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let attention_mask = Array2::from_shape_vec((batch, seq_len), input.attention_mask.clone())
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let token_type_ids = Array2::from_shape_vec((batch, seq_len), input.token_type_ids.clone())
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let words_mask = Array2::from_shape_vec((batch, seq_len), input.words_mask.clone())
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let text_lengths = Array1::from_vec(input.text_lengths.clone());

        // span_idx: [batch, num_spans, 2]
        let span_flat: Vec<i64> = input
            .span_idx
            .iter()
            .flat_map(|s| s.iter().copied())
            .collect();
        let span_idx = Array3::from_shape_vec((batch, num_spans, 2), span_flat)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let span_mask = Array2::from_shape_vec((batch, num_spans), input.span_mask.clone())
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        // Build tensor refs
        let input_ids_t = TensorRef::from_array_view(&input_ids)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let attention_mask_t = TensorRef::from_array_view(&attention_mask)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let token_type_ids_t = TensorRef::from_array_view(&token_type_ids)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let words_mask_t = TensorRef::from_array_view(&words_mask)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let text_lengths_t = TensorRef::from_array_view(&text_lengths)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let span_idx_t = TensorRef::from_array_view(&span_idx)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let span_mask_t = TensorRef::from_array_view(&span_mask)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        // Run inference
        let outputs = session.run(ort::inputs![
            "input_ids" => input_ids_t,
            "attention_mask" => attention_mask_t,
            "token_type_ids" => token_type_ids_t,
            "words_mask" => words_mask_t,
            "text_lengths" => text_lengths_t,
            "span_idx" => span_idx_t,
            "span_mask" => span_mask_t
        ])?;

        // Extract logits from output
        let output = if let Some(out) = outputs.get("logits") {
            out
        } else {
            &outputs[0]
        };

        let (_, logits) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| MlError::Postprocessing(e.to_string()))?;

        Ok(logits.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gliner_model_without_onnx() {
        let config = GlinerModelConfig::medium();
        let model = GlinerModel::new(config, ModelTier::Medium).unwrap();
        assert!(!model.has_model());
        assert_eq!(model.tier(), ModelTier::Medium);
    }

    #[test]
    fn test_gliner_model_extract_without_model() {
        let config = GlinerModelConfig::medium();
        let model = GlinerModel::new(config, ModelTier::Medium).unwrap();

        let (entities, _) = model
            .extract("Invoice INV-001 from Acme", &["invoice_number"], 0.5)
            .unwrap();

        assert!(entities.is_empty()); // no model loaded
    }
}
