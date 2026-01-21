use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use image::{DynamicImage, GenericImageView};
use ndarray::{Array2, Array3, Array4};
use ort::session::Session;
use ort::value::TensorRef;
use tokenizers::Tokenizer;

use super::{
    EntityType, KeyValuePair, LayoutLabel, LayoutModelConfig, LayoutRegion, LayoutResult,
    NamedEntity,
};
use crate::error::MlError;
use crate::ocr::{OcrResult, TextRegion};

/// LayoutLMv3-based document understanding model
pub struct LayoutModel {
    config: LayoutModelConfig,
    session: Option<Mutex<Session>>,
    tokenizer: Option<Tokenizer>,
}

impl LayoutModel {
    /// Create layout model without ONNX model (rule-based fallback)
    pub fn new(config: LayoutModelConfig) -> Result<Self, MlError> {
        Ok(Self {
            config,
            session: None,
            tokenizer: None,
        })
    }

    /// Create layout model with ONNX model and tokenizer
    pub fn with_model(
        config: LayoutModelConfig,
        model_path: impl AsRef<Path>,
        tokenizer_path: impl AsRef<Path>,
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
            "Loaded layout model"
        );

        Ok(Self {
            config,
            session: Some(Mutex::new(session)),
            tokenizer: Some(tokenizer),
        })
    }

    /// Check if ONNX model is loaded
    pub fn has_model(&self) -> bool {
        self.session.is_some() && self.tokenizer.is_some()
    }

    /// Analyze document layout
    pub fn analyze(
        &self,
        image: &DynamicImage,
        ocr_result: &OcrResult,
    ) -> Result<LayoutResult, MlError> {
        let start = Instant::now();

        // If model is loaded, use it; otherwise use rule-based analysis
        let result = if self.has_model() {
            self.analyze_with_model(image, ocr_result)?
        } else {
            self.analyze_rule_based(ocr_result)?
        };

        Ok(LayoutResult {
            processing_time_ms: start.elapsed().as_millis() as u64,
            ..result
        })
    }

    /// Analyze with ONNX model
    fn analyze_with_model(
        &self,
        image: &DynamicImage,
        ocr_result: &OcrResult,
    ) -> Result<LayoutResult, MlError> {
        let mut session = self
            .session
            .as_ref()
            .ok_or_else(|| MlError::ModelLoading("Layout model not loaded".to_string()))?
            .lock()
            .map_err(|e| MlError::ModelLoading(format!("Failed to acquire session lock: {}", e)))?;

        // Prepare inputs for LayoutLMv3
        let (input_ids, attention_mask, bbox, pixel_values) =
            self.prepare_inputs(image, ocr_result)?;

        // Create tensor references from arrays
        let input_ids_tensor = TensorRef::from_array_view(&input_ids)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let attention_mask_tensor = TensorRef::from_array_view(&attention_mask)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let bbox_tensor = TensorRef::from_array_view(&bbox)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let pixel_values_tensor = TensorRef::from_array_view(&pixel_values)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        // Run inference
        let outputs = session.run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
            "bbox" => bbox_tensor,
            "pixel_values" => pixel_values_tensor
        ])?;

        // Parse outputs - LayoutLMv3 outputs token classifications
        let output = if let Some(out) = outputs.get("logits") {
            out
        } else {
            &outputs[0]
        };

        let (_, logits) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| MlError::Postprocessing(e.to_string()))?;

        // Parse token-level predictions into regions and entities
        self.parse_layout_predictions(logits, ocr_result)
    }

    /// Prepare model inputs from image and OCR results
    fn prepare_inputs(
        &self,
        image: &DynamicImage,
        ocr_result: &OcrResult,
    ) -> Result<(Array2<i64>, Array2<i64>, Array3<i64>, Array4<f32>), MlError> {
        let max_len = self.config.max_seq_length;
        let img_size = self.config.image_size;

        let tokenizer = self
            .tokenizer
            .as_ref()
            .ok_or_else(|| MlError::Tokenization("Tokenizer not loaded".to_string()))?;

        // Tokenize text using the proper tokenizer
        let encoding = tokenizer
            .encode(ocr_result.text.as_str(), true)
            .map_err(|e| MlError::Tokenization(e.to_string()))?;

        let token_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let num_tokens = token_ids.len().min(max_len);

        // Input IDs (token indices)
        let mut input_ids = Array2::<i64>::zeros((1, max_len));
        for (i, &token) in token_ids.iter().take(max_len).enumerate() {
            input_ids[[0, i]] = token;
        }

        // Attention mask
        let mut attention_mask = Array2::<i64>::zeros((1, max_len));
        for i in 0..num_tokens {
            attention_mask[[0, i]] = 1;
        }

        // Bounding boxes (normalized 0-1000)
        let mut bbox = Array3::<i64>::zeros((1, max_len, 4));
        let (img_w, img_h) = image.dimensions();

        // Map word pieces back to OCR regions
        let word_ids = encoding.get_word_ids();
        for (token_idx, word_id) in word_ids.iter().enumerate() {
            if token_idx >= max_len {
                break;
            }

            if let Some(word_idx) = word_id {
                // Find corresponding OCR region for this word
                if let Some(region) = ocr_result.regions.get(*word_idx as usize) {
                    let x0 = ((region.bbox.x / img_w as f32) * 1000.0) as i64;
                    let y0 = ((region.bbox.y / img_h as f32) * 1000.0) as i64;
                    let x1 = (((region.bbox.x + region.bbox.width) / img_w as f32) * 1000.0) as i64;
                    let y1 =
                        (((region.bbox.y + region.bbox.height) / img_h as f32) * 1000.0) as i64;

                    bbox[[0, token_idx, 0]] = x0.clamp(0, 1000);
                    bbox[[0, token_idx, 1]] = y0.clamp(0, 1000);
                    bbox[[0, token_idx, 2]] = x1.clamp(0, 1000);
                    bbox[[0, token_idx, 3]] = y1.clamp(0, 1000);
                }
            }
        }

        // Pixel values (resized and normalized image)
        let resized =
            image.resize_exact(img_size, img_size, image::imageops::FilterType::Lanczos3);
        let rgb = resized.to_rgb8();

        let mut pixel_values = Array4::<f32>::zeros((1, 3, img_size as usize, img_size as usize));

        // ImageNet normalization
        let mean = [0.485, 0.456, 0.406];
        let std = [0.229, 0.224, 0.225];

        for (x, y, pixel) in rgb.enumerate_pixels() {
            let x = x as usize;
            let y = y as usize;
            pixel_values[[0, 0, y, x]] = (pixel[0] as f32 / 255.0 - mean[0]) / std[0];
            pixel_values[[0, 1, y, x]] = (pixel[1] as f32 / 255.0 - mean[1]) / std[1];
            pixel_values[[0, 2, y, x]] = (pixel[2] as f32 / 255.0 - mean[2]) / std[2];
        }

        Ok((input_ids, attention_mask, bbox, pixel_values))
    }

    /// Parse model predictions into layout result
    fn parse_layout_predictions(
        &self,
        logits: &[f32],
        ocr_result: &OcrResult,
    ) -> Result<LayoutResult, MlError> {
        let num_labels = 13; // Number of layout labels
        let seq_len = logits.len() / num_labels;

        let mut regions = Vec::new();
        let mut entities = Vec::new();

        for (i, region) in ocr_result.regions.iter().enumerate() {
            if i >= seq_len {
                break;
            }

            // Get predicted label for this token
            let start_idx = i * num_labels;
            let end_idx = (start_idx + num_labels).min(logits.len());
            let token_logits = &logits[start_idx..end_idx];

            let (label_idx, confidence) = self.argmax_with_softmax(token_logits);
            let label = LayoutLabel::from_index(label_idx);

            regions.push(LayoutRegion {
                label,
                bbox: region.bbox,
                confidence,
                text: region.text.clone(),
                children: Vec::new(),
            });

            // Extract named entities based on label and text
            if let Some(entity) = self.extract_entity_from_region(&region.text, label, region) {
                entities.push(entity);
            }
        }

        Ok(LayoutResult {
            regions,
            entities,
            key_value_pairs: self.extract_key_value_pairs(ocr_result),
            text: ocr_result.text.clone(),
            processing_time_ms: 0,
        })
    }

    /// Get argmax with softmax probability
    fn argmax_with_softmax(&self, logits: &[f32]) -> (usize, f32) {
        if logits.is_empty() {
            return (0, 0.0);
        }

        let max_val = logits.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        let exp_sum: f32 = logits.iter().map(|&x| (x - max_val).exp()).sum();

        let mut best_idx = 0;
        let mut best_prob = 0.0;

        for (i, &val) in logits.iter().enumerate() {
            let prob = (val - max_val).exp() / exp_sum;
            if prob > best_prob {
                best_prob = prob;
                best_idx = i;
            }
        }

        (best_idx, best_prob)
    }

    /// Extract entity from a text region based on label
    fn extract_entity_from_region(
        &self,
        text: &str,
        label: LayoutLabel,
        region: &TextRegion,
    ) -> Option<NamedEntity> {
        // Map layout labels to entity types for certain regions
        let entity_type = match label {
            LayoutLabel::FormField => {
                // Analyze text content to determine entity type
                self.infer_entity_type(text)
            }
            LayoutLabel::Header => Some(EntityType::Organization),
            _ => None,
        };

        entity_type.map(|et| {
            NamedEntity::new(et, text.to_string(), region.confidence, 0, text.len())
                .with_bbox(region.bbox)
        })
    }

    /// Infer entity type from text content
    fn infer_entity_type(&self, text: &str) -> Option<EntityType> {
        let lower = text.to_lowercase();

        // Date patterns
        if lower.contains("date")
            || text.contains('/')
            || text.contains('-')
                && text.chars().filter(|c| c.is_ascii_digit()).count() >= 4
        {
            return Some(EntityType::Date);
        }

        // Amount patterns
        if text.contains('$')
            || text.contains('€')
            || text.contains('£')
            || lower.contains("total")
            || lower.contains("amount")
        {
            return Some(EntityType::Amount);
        }

        // Invoice number patterns
        if lower.contains("invoice") || lower.contains("inv-") || lower.contains("inv#") {
            return Some(EntityType::InvoiceNumber);
        }

        None
    }

    /// Rule-based layout analysis (fallback when no model)
    fn analyze_rule_based(&self, ocr_result: &OcrResult) -> Result<LayoutResult, MlError> {
        let mut regions = Vec::new();
        let mut entities = Vec::new();

        for region in &ocr_result.regions {
            // Classify region based on position and content
            let label = self.classify_region_rule_based(region, ocr_result);

            regions.push(LayoutRegion {
                label,
                bbox: region.bbox,
                confidence: region.confidence,
                text: region.text.clone(),
                children: Vec::new(),
            });

            // Extract entities using regex patterns
            entities.extend(self.extract_entities_rule_based(&region.text, region));
        }

        Ok(LayoutResult {
            regions,
            entities,
            key_value_pairs: self.extract_key_value_pairs(ocr_result),
            text: ocr_result.text.clone(),
            processing_time_ms: 0,
        })
    }

    /// Classify region using heuristic rules
    fn classify_region_rule_based(
        &self,
        region: &TextRegion,
        _ocr_result: &OcrResult,
    ) -> LayoutLabel {
        let text = &region.text;
        let lower = text.to_lowercase();

        // Check for title (large text at top)
        if region.bbox.y < 100.0 && region.bbox.height > 20.0 {
            if text.len() < 100 && !text.contains('\n') {
                return LayoutLabel::Title;
            }
        }

        // Check for header/footer by position
        if region.bbox.y < 50.0 {
            return LayoutLabel::Header;
        }

        // Check for table-like content
        if text.contains('\t') || (text.matches(' ').count() > 10 && text.lines().count() > 2) {
            return LayoutLabel::Table;
        }

        // Check for list
        if lower.starts_with("• ")
            || lower.starts_with("- ")
            || lower.starts_with("* ")
            || text.starts_with("1.")
        {
            return LayoutLabel::List;
        }

        // Check for form fields
        if lower.contains(':') && text.len() < 100 {
            return LayoutLabel::FormField;
        }

        LayoutLabel::Text
    }

    /// Extract entities using regex patterns
    fn extract_entities_rule_based(&self, text: &str, region: &TextRegion) -> Vec<NamedEntity> {
        let mut entities = Vec::new();

        // Date patterns (MM/DD/YYYY, DD-MM-YYYY, etc.)
        let date_pattern =
            regex::Regex::new(r"\b(\d{1,2}[/-]\d{1,2}[/-]\d{2,4}|\d{4}[/-]\d{1,2}[/-]\d{1,2})\b")
                .ok();
        if let Some(re) = date_pattern {
            for m in re.find_iter(text) {
                entities.push(
                    NamedEntity::new(
                        EntityType::Date,
                        m.as_str().to_string(),
                        0.9,
                        m.start(),
                        m.end(),
                    )
                    .with_bbox(region.bbox),
                );
            }
        }

        // Amount patterns ($1,234.56, etc.)
        let amount_pattern = regex::Regex::new(
            r"[$€£]\s*[\d,]+\.?\d*|\d{1,3}(?:,\d{3})*(?:\.\d{2})?(?:\s*(?:USD|EUR|GBP))?",
        )
        .ok();
        if let Some(re) = amount_pattern {
            for m in re.find_iter(text) {
                if m.as_str().chars().any(|c| c.is_ascii_digit()) {
                    entities.push(
                        NamedEntity::new(
                            EntityType::Amount,
                            m.as_str().to_string(),
                            0.85,
                            m.start(),
                            m.end(),
                        )
                        .with_bbox(region.bbox),
                    );
                }
            }
        }

        // Invoice number patterns
        let invoice_pattern =
            regex::Regex::new(r"(?i)(?:INV|Invoice)[#\-:\s]*([A-Z0-9\-]+)").ok();
        if let Some(re) = invoice_pattern {
            for cap in re.captures_iter(text) {
                if let Some(m) = cap.get(1) {
                    entities.push(
                        NamedEntity::new(
                            EntityType::InvoiceNumber,
                            m.as_str().to_string(),
                            0.9,
                            m.start(),
                            m.end(),
                        )
                        .with_bbox(region.bbox),
                    );
                }
            }
        }

        // Email patterns
        let email_pattern =
            regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").ok();
        if let Some(re) = email_pattern {
            for m in re.find_iter(text) {
                entities.push(
                    NamedEntity::new(
                        EntityType::Email,
                        m.as_str().to_string(),
                        0.95,
                        m.start(),
                        m.end(),
                    )
                    .with_bbox(region.bbox),
                );
            }
        }

        entities
    }

    /// Extract key-value pairs from OCR result
    fn extract_key_value_pairs(&self, ocr_result: &OcrResult) -> Vec<KeyValuePair> {
        let mut pairs = Vec::new();

        // Look for patterns like "Key: Value" or "Key Value" with aligned positions
        let kv_pattern = regex::Regex::new(r"([A-Za-z][A-Za-z\s]*?):\s*(.+)").ok();

        if let Some(re) = kv_pattern {
            for region in &ocr_result.regions {
                for cap in re.captures_iter(&region.text) {
                    if let (Some(key), Some(value)) = (cap.get(1), cap.get(2)) {
                        let key_text = key.as_str().trim();
                        let value_text = value.as_str().trim();

                        if !key_text.is_empty() && !value_text.is_empty() {
                            pairs.push(KeyValuePair {
                                key: key_text.to_string(),
                                value: value_text.to_string(),
                                confidence: region.confidence,
                                key_bbox: Some(region.bbox),
                                value_bbox: Some(region.bbox),
                            });
                        }
                    }
                }
            }
        }

        pairs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_model_without_onnx() {
        let config = LayoutModelConfig::default();
        let model = LayoutModel::new(config).unwrap();
        assert!(!model.has_model());
    }

    #[test]
    fn test_layout_label_roundtrip() {
        for i in 0..13 {
            let label = LayoutLabel::from_index(i);
            assert_eq!(label.to_index(), i);
        }
    }

    #[test]
    fn test_entity_type_from_label() {
        assert_eq!(
            EntityType::from_label("invoice_number"),
            EntityType::InvoiceNumber
        );
        assert_eq!(EntityType::from_label("date"), EntityType::Date);
        assert_eq!(EntityType::from_label("amount"), EntityType::Amount);
        assert_eq!(EntityType::from_label("unknown"), EntityType::Other);
    }
}
