use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use async_trait::async_trait;
use image::{DynamicImage, GenericImageView};
use ndarray::Array4;
use ort::session::Session;
use ort::value::TensorRef;

use super::{BoundingBox, CtcDecodingStrategy, OcrConfig, OcrResult, TextRegion};
use crate::error::MlError;

/// Trait for OCR engines
#[async_trait]
pub trait OcrProvider: Send + Sync {
    /// Process an image and extract text
    async fn process_image(&self, image: &DynamicImage) -> Result<OcrResult, MlError>;

    /// Process raw image bytes
    async fn process_bytes(&self, bytes: &[u8]) -> Result<OcrResult, MlError>;
}

/// ONNX-based OCR Engine
/// Implements a two-stage pipeline: text detection + text recognition
pub struct OcrEngine {
    config: OcrConfig,
    detection_session: Option<Mutex<Session>>,
    recognition_session: Option<Mutex<Session>>,
}

impl OcrEngine {
    /// Create a new OCR engine without models (returns empty results)
    pub fn new(config: OcrConfig) -> Result<Self, MlError> {
        Ok(Self {
            config,
            detection_session: None,
            recognition_session: None,
        })
    }

    /// Create OCR engine with ONNX models
    pub fn with_models(
        config: OcrConfig,
        detection_model_path: impl AsRef<Path>,
        recognition_model_path: impl AsRef<Path>,
    ) -> Result<Self, MlError> {
        let detection_path = detection_model_path.as_ref();
        let recognition_path = recognition_model_path.as_ref();

        if !detection_path.exists() {
            return Err(MlError::ModelNotFound(
                detection_path.display().to_string(),
            ));
        }
        if !recognition_path.exists() {
            return Err(MlError::ModelNotFound(
                recognition_path.display().to_string(),
            ));
        }

        let detection_session = Session::builder()?.commit_from_file(detection_path)?;

        let recognition_session = Session::builder()?.commit_from_file(recognition_path)?;

        tracing::info!(
            detection_model = %detection_path.display(),
            recognition_model = %recognition_path.display(),
            "Loaded OCR models"
        );

        Ok(Self {
            config,
            detection_session: Some(Mutex::new(detection_session)),
            recognition_session: Some(Mutex::new(recognition_session)),
        })
    }

    /// Check if models are loaded
    pub fn has_models(&self) -> bool {
        self.detection_session.is_some() && self.recognition_session.is_some()
    }

    /// Preprocess image for detection model
    fn preprocess_for_detection(&self, image: &DynamicImage) -> Result<Array4<f32>, MlError> {
        let (width, height) = image.dimensions();

        // Resize if needed
        let (new_width, new_height) = if width > self.config.max_dimension
            || height > self.config.max_dimension
        {
            let scale = self.config.max_dimension as f32 / width.max(height) as f32;
            (
                (width as f32 * scale) as u32,
                (height as f32 * scale) as u32,
            )
        } else {
            (width, height)
        };

        let resized = image.resize_exact(
            new_width,
            new_height,
            image::imageops::FilterType::Lanczos3,
        );
        let rgb = resized.to_rgb8();

        // Convert to NCHW format with normalization
        let mut arr = Array4::<f32>::zeros((1, 3, new_height as usize, new_width as usize));

        for (x, y, pixel) in rgb.enumerate_pixels() {
            let x = x as usize;
            let y = y as usize;
            arr[[0, 0, y, x]] = pixel[0] as f32 / 255.0;
            arr[[0, 1, y, x]] = pixel[1] as f32 / 255.0;
            arr[[0, 2, y, x]] = pixel[2] as f32 / 255.0;
        }

        Ok(arr)
    }

    /// Preprocess a text region for recognition
    fn preprocess_for_recognition(
        &self,
        image: &DynamicImage,
        bbox: &BoundingBox,
    ) -> Result<Array4<f32>, MlError> {
        // Crop the region
        let x = bbox.x as u32;
        let y = bbox.y as u32;
        let w = bbox.width as u32;
        let h = bbox.height as u32;

        let cropped = image.crop_imm(x, y, w.max(1), h.max(1));

        // Resize to recognition model input size (typically 32xN for CRNN-based models)
        let target_height = 32u32;
        let aspect_ratio = cropped.width() as f32 / cropped.height() as f32;
        let target_width = ((target_height as f32 * aspect_ratio) as u32).max(1);

        let resized = cropped.resize_exact(
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        );
        let rgb = resized.to_rgb8();

        // Convert to NCHW format
        let mut arr = Array4::<f32>::zeros((1, 3, target_height as usize, target_width as usize));

        for (x, y, pixel) in rgb.enumerate_pixels() {
            let x = x as usize;
            let y = y as usize;
            arr[[0, 0, y, x]] = (pixel[0] as f32 / 255.0 - 0.5) / 0.5;
            arr[[0, 1, y, x]] = (pixel[1] as f32 / 255.0 - 0.5) / 0.5;
            arr[[0, 2, y, x]] = (pixel[2] as f32 / 255.0 - 0.5) / 0.5;
        }

        Ok(arr)
    }

    /// Run text detection on preprocessed image
    fn detect_text_regions(
        &self,
        image: &DynamicImage,
        input: Array4<f32>,
    ) -> Result<Vec<BoundingBox>, MlError> {
        let mut detection_session = self
            .detection_session
            .as_ref()
            .ok_or_else(|| MlError::ModelLoading("Detection model not loaded".to_string()))?
            .lock()
            .map_err(|e| MlError::ModelLoading(format!("Failed to acquire session lock: {}", e)))?;

        let input_tensor = TensorRef::from_array_view(&input)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let outputs = detection_session.run(ort::inputs!["input" => input_tensor])?;

        // Parse detection output - format depends on model
        let output = if let Some(out) = outputs.get("output") {
            out
        } else {
            &outputs[0]
        };

        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| MlError::Postprocessing(e.to_string()))?;

        let (img_width, img_height) = image.dimensions();
        let mut boxes = Vec::new();

        // Parse output based on model format (assuming YOLO-style output)
        // [batch, num_boxes, 5+num_classes] where 5 = x, y, w, h, confidence
        // Shape derefs to [i64] slice

        if shape.len() >= 2 {
            let num_boxes = if shape.len() == 3 { shape[1] as usize } else { shape[0] as usize };
            let stride = if shape.len() == 3 { shape[2] as usize } else { 5 };

            for i in 0..num_boxes {
                let offset = i * stride;
                if offset + 4 >= data.len() {
                    break;
                }

                let x = data[offset];
                let y = data[offset + 1];
                let w = data[offset + 2];
                let h = data[offset + 3];
                let conf = if offset + 4 < data.len() { data[offset + 4] } else { 1.0 };

                if conf > self.config.confidence_threshold {
                    boxes.push(BoundingBox::new(
                        x * img_width as f32,
                        y * img_height as f32,
                        w * img_width as f32,
                        h * img_height as f32,
                    ));
                }
            }
        }

        // Apply NMS (Non-Maximum Suppression)
        let boxes = self.non_max_suppression(boxes, 0.5);

        Ok(boxes)
    }

    /// Non-Maximum Suppression to remove overlapping boxes
    fn non_max_suppression(&self, mut boxes: Vec<BoundingBox>, iou_threshold: f32) -> Vec<BoundingBox> {
        // Sort by area (larger boxes first)
        boxes.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());

        let mut keep = Vec::new();

        while !boxes.is_empty() {
            let current = boxes.remove(0);
            keep.push(current);

            boxes.retain(|b| current.iou(b) < iou_threshold);
        }

        keep
    }

    /// Recognize text in a region (single)
    fn recognize_text(
        &self,
        input: Array4<f32>,
    ) -> Result<(String, f32), MlError> {
        let results = self.recognize_text_batch(&[input])?;
        Ok(results.into_iter().next().unwrap_or_else(|| (String::new(), 0.0)))
    }

    /// Recognize text in multiple regions with batched inference
    /// This is significantly faster than processing regions one at a time
    fn recognize_text_batch(
        &self,
        inputs: &[Array4<f32>],
    ) -> Result<Vec<(String, f32)>, MlError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        let mut recognition_session = self
            .recognition_session
            .as_ref()
            .ok_or_else(|| MlError::ModelLoading("Recognition model not loaded".to_string()))?
            .lock()
            .map_err(|e| MlError::ModelLoading(format!("Failed to acquire session lock: {}", e)))?;

        let batch_size = inputs.len();

        // Get dimensions from first input (all inputs should have same dimensions)
        let first = &inputs[0];
        let (_, channels, height, width) = (
            first.shape()[0],
            first.shape()[1],
            first.shape()[2],
            first.shape()[3],
        );

        // Combine all inputs into a single batch tensor
        let mut batch_input = Array4::<f32>::zeros((batch_size, channels, height, width));
        for (i, input) in inputs.iter().enumerate() {
            batch_input
                .slice_mut(ndarray::s![i, .., .., ..])
                .assign(&input.slice(ndarray::s![0, .., .., ..]));
        }

        let input_tensor = TensorRef::from_array_view(&batch_input)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let outputs = recognition_session.run(ort::inputs!["input" => input_tensor])?;

        let output = if let Some(out) = outputs.get("output") {
            out
        } else {
            &outputs[0]
        };

        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| MlError::Postprocessing(e.to_string()))?;

        // Parse batch output - shape should be [batch, seq_len, vocab_size] or [batch * seq_len, vocab_size]
        let mut results = Vec::with_capacity(batch_size);

        if shape.len() == 3 {
            // Shape: [batch, seq_len, vocab_size]
            let seq_len = shape[1] as usize;
            let vocab_size = shape[2] as usize;
            let sample_size = seq_len * vocab_size;

            for batch_idx in 0..batch_size {
                let start = batch_idx * sample_size;
                let end = start + sample_size;
                if end <= data.len() {
                    let sample_data = &data[start..end];
                    let (text, confidence) = self.decode_ctc(sample_data);
                    results.push((text, confidence));
                } else {
                    results.push((String::new(), 0.0));
                }
            }
        } else {
            // Fallback: process as single output
            let (text, confidence) = self.decode_ctc(data);
            results.push((text, confidence));
        }

        Ok(results)
    }

    /// Get the vocabulary/charset for CTC decoding
    fn get_vocabulary(&self) -> Vec<char> {
        self.config
            .decoder_config
            .vocabulary
            .clone()
            .unwrap_or_else(|| {
                // Default vocabulary: printable ASCII + common extended characters
                // Index 0 is reserved for blank token
                let mut chars: Vec<char> = Vec::with_capacity(128);
                // Space and punctuation
                chars.extend(" !\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".chars());
                // Digits
                chars.extend('0'..='9');
                // Uppercase letters
                chars.extend('A'..='Z');
                // Lowercase letters
                chars.extend('a'..='z');
                // Common currency and symbols
                chars.extend("£€¥¢©®™°±×÷".chars());
                chars
            })
    }

    /// Decode CTC output to text using configured strategy
    fn decode_ctc(&self, logits: &[f32]) -> (String, f32) {
        let vocabulary = self.get_vocabulary();
        let vocab_size = vocabulary.len() + 1; // +1 for blank token at index 0
        let seq_len = logits.len() / vocab_size;

        if seq_len == 0 {
            return (String::new(), 0.0);
        }

        // Convert logits to log probabilities for numerical stability
        let log_probs = self.compute_log_probabilities(logits, vocab_size, seq_len);

        match self.config.decoder_config.strategy {
            CtcDecodingStrategy::Greedy => {
                self.decode_ctc_greedy(&log_probs, &vocabulary, vocab_size, seq_len)
            }
            CtcDecodingStrategy::BeamSearch => {
                self.decode_ctc_beam_search(&log_probs, &vocabulary, vocab_size, seq_len)
            }
        }
    }

    /// Compute log probabilities from logits with numerical stability
    fn compute_log_probabilities(
        &self,
        logits: &[f32],
        vocab_size: usize,
        seq_len: usize,
    ) -> Vec<Vec<f32>> {
        let mut log_probs = Vec::with_capacity(seq_len);

        for t in 0..seq_len {
            let start = t * vocab_size;
            let end = (start + vocab_size).min(logits.len());
            let frame = &logits[start..end];

            // Log-softmax for numerical stability
            let max_val = frame.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            let log_sum_exp: f32 = frame
                .iter()
                .map(|&x| (x - max_val).exp())
                .sum::<f32>()
                .ln()
                + max_val;

            let frame_log_probs: Vec<f32> = frame
                .iter()
                .map(|&x| x - log_sum_exp)
                .collect();

            log_probs.push(frame_log_probs);
        }

        log_probs
    }

    /// Greedy CTC decoding - fastest but less accurate
    fn decode_ctc_greedy(
        &self,
        log_probs: &[Vec<f32>],
        vocabulary: &[char],
        _vocab_size: usize,
        seq_len: usize,
    ) -> (String, f32) {
        let blank_idx = self.config.decoder_config.blank_index;
        let mut text = String::new();
        let mut prev_idx: Option<usize> = None;
        let mut total_log_prob = 0.0;
        let mut char_count = 0;

        for t in 0..seq_len {
            let frame = &log_probs[t];

            // Find argmax
            let (best_idx, best_log_prob) = frame
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .unwrap_or((blank_idx, &f32::NEG_INFINITY));

            // CTC collapse: skip blank and repeated characters
            if best_idx != blank_idx && Some(best_idx) != prev_idx {
                // Convert index to character (accounting for blank at index 0)
                let char_idx = if best_idx > blank_idx {
                    best_idx - 1
                } else {
                    best_idx
                };

                if let Some(&c) = vocabulary.get(char_idx) {
                    text.push(c);
                    total_log_prob += best_log_prob;
                    char_count += 1;
                }
            }

            prev_idx = Some(best_idx);
        }

        // Convert log probability to confidence score
        let avg_confidence = if char_count > 0 {
            (total_log_prob / char_count as f32).exp()
        } else {
            0.0
        };

        (text, avg_confidence)
    }

    /// Beam search CTC decoding - more accurate but slower
    fn decode_ctc_beam_search(
        &self,
        log_probs: &[Vec<f32>],
        vocabulary: &[char],
        vocab_size: usize,
        seq_len: usize,
    ) -> (String, f32) {
        let blank_idx = self.config.decoder_config.blank_index;
        let beam_width = self.config.decoder_config.beam_width;

        // Beam: (prefix, log_prob_blank, log_prob_non_blank)
        // We track two probabilities: ending in blank vs ending in non-blank
        let mut beams: Vec<(String, f32, f32)> = vec![(String::new(), 0.0, f32::NEG_INFINITY)];

        for t in 0..seq_len {
            let frame = &log_probs[t];
            let mut new_beams: std::collections::HashMap<String, (f32, f32)> =
                std::collections::HashMap::new();

            for (prefix, log_pb, log_pnb) in &beams {
                let log_p = self.log_add(*log_pb, *log_pnb);

                // Process blank
                let blank_log_prob = frame.get(blank_idx).copied().unwrap_or(f32::NEG_INFINITY);
                let entry = new_beams.entry(prefix.clone()).or_insert((f32::NEG_INFINITY, f32::NEG_INFINITY));
                entry.0 = self.log_add(entry.0, log_p + blank_log_prob);

                // Process each character
                for c_idx in 0..vocab_size {
                    if c_idx == blank_idx {
                        continue;
                    }

                    let c_log_prob = frame.get(c_idx).copied().unwrap_or(f32::NEG_INFINITY);
                    let char_idx = if c_idx > blank_idx { c_idx - 1 } else { c_idx };

                    if let Some(&c) = vocabulary.get(char_idx) {
                        let last_char = prefix.chars().last();

                        if Some(c) == last_char {
                            // Same character: can only extend if previous was blank
                            let mut new_prefix = prefix.clone();
                            new_prefix.push(c);
                            let entry = new_beams.entry(new_prefix).or_insert((f32::NEG_INFINITY, f32::NEG_INFINITY));
                            entry.1 = self.log_add(entry.1, *log_pb + c_log_prob);

                            // Or stay with current prefix
                            let entry = new_beams.entry(prefix.clone()).or_insert((f32::NEG_INFINITY, f32::NEG_INFINITY));
                            entry.1 = self.log_add(entry.1, *log_pnb + c_log_prob);
                        } else {
                            // Different character: can always extend
                            let mut new_prefix = prefix.clone();
                            new_prefix.push(c);
                            let entry = new_beams.entry(new_prefix).or_insert((f32::NEG_INFINITY, f32::NEG_INFINITY));
                            entry.1 = self.log_add(entry.1, log_p + c_log_prob);
                        }
                    }
                }
            }

            // Convert back to beam format and prune
            beams = new_beams
                .into_iter()
                .map(|(prefix, (log_pb, log_pnb))| (prefix, log_pb, log_pnb))
                .collect();

            // Sort by total probability and keep top beams
            beams.sort_by(|a, b| {
                let prob_a = self.log_add(a.1, a.2);
                let prob_b = self.log_add(b.1, b.2);
                prob_b.partial_cmp(&prob_a).unwrap()
            });
            beams.truncate(beam_width);
        }

        // Return best beam
        if let Some((text, log_pb, log_pnb)) = beams.into_iter().next() {
            let log_prob = self.log_add(log_pb, log_pnb);
            let confidence = log_prob.exp().min(1.0);
            (text, confidence)
        } else {
            (String::new(), 0.0)
        }
    }

    /// Log-space addition: log(exp(a) + exp(b))
    fn log_add(&self, a: f32, b: f32) -> f32 {
        if a == f32::NEG_INFINITY {
            return b;
        }
        if b == f32::NEG_INFINITY {
            return a;
        }
        let max = a.max(b);
        max + ((a - max).exp() + (b - max).exp()).ln()
    }

    /// Sort text regions by reading order (top-to-bottom, left-to-right)
    fn sort_by_reading_order(&self, regions: &mut Vec<TextRegion>) {
        regions.sort_by(|a, b| {
            let y_diff = a.bbox.y - b.bbox.y;
            // If on roughly the same line (within 10 pixels), sort by x
            if y_diff.abs() < 10.0 {
                a.bbox.x.partial_cmp(&b.bbox.x).unwrap()
            } else {
                a.bbox.y.partial_cmp(&b.bbox.y).unwrap()
            }
        });

        // Update order indices
        for (i, region) in regions.iter_mut().enumerate() {
            region.order = i;
        }
    }
}

#[async_trait]
impl OcrProvider for OcrEngine {
    async fn process_image(&self, image: &DynamicImage) -> Result<OcrResult, MlError> {
        let start = Instant::now();

        // If no models loaded, return empty result
        if !self.has_models() {
            tracing::debug!("No OCR models loaded, returning empty result");
            return Ok(OcrResult {
                text: String::new(),
                regions: Vec::new(),
                confidence: 0.0,
                processing_time_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Step 1: Preprocess for detection
        let detection_input = self.preprocess_for_detection(image)?;

        // Step 2: Detect text regions
        let boxes = self.detect_text_regions(image, detection_input)?;

        if boxes.is_empty() {
            return Ok(OcrResult {
                text: String::new(),
                regions: Vec::new(),
                confidence: 0.0,
                processing_time_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Step 3: Preprocess all regions for batch recognition
        let recognition_inputs: Vec<Array4<f32>> = boxes
            .iter()
            .filter_map(|bbox| self.preprocess_for_recognition(image, bbox).ok())
            .collect();

        // Step 4: Batch recognize all text regions in a single forward pass
        let recognition_results = self.recognize_text_batch(&recognition_inputs)?;

        // Step 5: Build text regions from results
        let mut regions = Vec::with_capacity(boxes.len());
        let mut total_confidence = 0.0;

        for (i, ((bbox, _input), (text, confidence))) in boxes
            .iter()
            .zip(recognition_inputs.iter())
            .zip(recognition_results.into_iter())
            .enumerate()
        {
            if confidence >= self.config.confidence_threshold {
                regions.push(TextRegion {
                    text,
                    bbox: *bbox,
                    confidence,
                    order: i,
                });
                total_confidence += confidence;
            }
        }

        // Sort by reading order
        self.sort_by_reading_order(&mut regions);

        // Combine text
        let text = regions
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let avg_confidence = if !regions.is_empty() {
            total_confidence / regions.len() as f32
        } else {
            0.0
        };

        Ok(OcrResult {
            text,
            regions,
            confidence: avg_confidence,
            processing_time_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn process_bytes(&self, bytes: &[u8]) -> Result<OcrResult, MlError> {
        let image = image::load_from_memory(bytes)?;
        self.process_image(&image).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounding_box_iou() {
        let box1 = BoundingBox::new(0.0, 0.0, 100.0, 100.0);
        let box2 = BoundingBox::new(50.0, 50.0, 100.0, 100.0);

        let iou = box1.iou(&box2);
        assert!(iou > 0.0 && iou < 1.0);

        // Same box should have IoU = 1.0
        let box3 = BoundingBox::new(0.0, 0.0, 100.0, 100.0);
        assert!((box1.iou(&box3) - 1.0).abs() < 0.001);

        // Non-overlapping boxes should have IoU = 0.0
        let box4 = BoundingBox::new(200.0, 200.0, 50.0, 50.0);
        assert_eq!(box1.iou(&box4), 0.0);
    }

    #[test]
    fn test_ocr_engine_without_models() {
        let config = OcrConfig::default();
        let engine = OcrEngine::new(config).unwrap();
        assert!(!engine.has_models());
    }
}
