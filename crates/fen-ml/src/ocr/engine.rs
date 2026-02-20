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
            return Err(MlError::ModelNotFound(detection_path.display().to_string()));
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

    /// Round up to the nearest multiple of 32 (required by PaddleOCR detection model).
    fn round_to_32(val: u32) -> u32 {
        ((val + 31) / 32) * 32
    }

    /// Preprocess image for detection model
    #[tracing::instrument(skip(self, image), fields(width = %image.width(), height = %image.height()))]
    fn preprocess_for_detection(&self, image: &DynamicImage) -> Result<Array4<f32>, MlError> {
        let (width, height) = image.dimensions();

        // Resize if larger than max_dimension
        let (new_width, new_height) =
            if width > self.config.max_dimension || height > self.config.max_dimension {
                let scale = self.config.max_dimension as f32 / width.max(height) as f32;
                (
                    (width as f32 * scale) as u32,
                    (height as f32 * scale) as u32,
                )
            } else {
                (width, height)
            };

        // PaddleOCR detection model requires dimensions to be multiples of 32
        let padded_width = Self::round_to_32(new_width);
        let padded_height = Self::round_to_32(new_height);

        let resized =
            image.resize_exact(new_width, new_height, image::imageops::FilterType::Lanczos3);
        let rgb = resized.to_rgb8();

        // Convert to NCHW format with ImageNet normalization (required by PaddleOCR)
        // zero-padded to multiples of 32
        let mean = [0.485f32, 0.456, 0.406];
        let std = [0.229f32, 0.224, 0.225];
        let mut arr = Array4::<f32>::zeros((1, 3, padded_height as usize, padded_width as usize));

        for (x, y, pixel) in rgb.enumerate_pixels() {
            let x = x as usize;
            let y = y as usize;
            arr[[0, 0, y, x]] = (pixel[0] as f32 / 255.0 - mean[0]) / std[0];
            arr[[0, 1, y, x]] = (pixel[1] as f32 / 255.0 - mean[1]) / std[1];
            arr[[0, 2, y, x]] = (pixel[2] as f32 / 255.0 - mean[2]) / std[2];
        }

        Ok(arr)
    }

    /// Preprocess a text region for recognition
    #[tracing::instrument(skip(self, image), fields(bbox_x = %bbox.x, bbox_y = %bbox.y))]
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

        // Resize to recognition model input size (48xN for PaddleOCR SVTR model)
        let target_height = 48u32;
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
    #[tracing::instrument(skip(self, image, input), fields(input_shape = ?input.shape()))]
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

        let outputs = detection_session.run(ort::inputs!["x" => input_tensor])?;

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

        // PaddleOCR DBNet outputs a probability map: [batch, 1, map_h, map_w]
        // Each pixel value is the probability of being text.
        let (map_h, map_w) = if shape.len() == 4 {
            (shape[2] as usize, shape[3] as usize)
        } else if shape.len() == 3 {
            (shape[1] as usize, shape[2] as usize)
        } else {
            return Err(MlError::Postprocessing(format!(
                "Unexpected detection output shape: {:?}",
                shape
            )));
        };

        // Extract bounding boxes from the probability map using connected components
        let boxes = self.extract_boxes_from_prob_map(
            data,
            map_h,
            map_w,
            img_width,
            img_height,
            self.config.detection_threshold,
        );

        Ok(boxes)
    }

    /// Extract bounding boxes from DBNet probability map using simple thresholding
    /// and connected-component-like scanning.
    fn extract_boxes_from_prob_map(
        &self,
        data: &[f32],
        map_h: usize,
        map_w: usize,
        img_width: u32,
        img_height: u32,
        threshold: f32,
    ) -> Vec<BoundingBox> {
        // Binary threshold the probability map
        let mut visited = vec![false; map_h * map_w];
        let mut boxes = Vec::new();

        let scale_x = img_width as f32 / map_w as f32;
        let scale_y = img_height as f32 / map_h as f32;

        // Simple scan-line connected component extraction
        for y in 0..map_h {
            for x in 0..map_w {
                let idx = y * map_w + x;
                if idx >= data.len() || visited[idx] || data[idx] < threshold {
                    continue;
                }

                // Flood fill to find connected region
                let mut min_x = x;
                let mut min_y = y;
                let mut max_x = x;
                let mut max_y = y;
                let mut score_sum = 0.0f32;
                let mut count = 0usize;

                let mut stack = vec![(x, y)];
                while let Some((cx, cy)) = stack.pop() {
                    let cidx = cy * map_w + cx;
                    if cx >= map_w || cy >= map_h || cidx >= data.len() || visited[cidx] || data[cidx] < threshold {
                        continue;
                    }
                    visited[cidx] = true;
                    score_sum += data[cidx];
                    count += 1;
                    min_x = min_x.min(cx);
                    min_y = min_y.min(cy);
                    max_x = max_x.max(cx);
                    max_y = max_y.max(cy);

                    // 4-connected neighbors
                    if cx > 0 { stack.push((cx - 1, cy)); }
                    if cx + 1 < map_w { stack.push((cx + 1, cy)); }
                    if cy > 0 { stack.push((cx, cy - 1)); }
                    if cy + 1 < map_h { stack.push((cx, cy + 1)); }
                }

                // Filter out very small regions (noise)
                let region_w = max_x - min_x + 1;
                let region_h = max_y - min_y + 1;
                if count < 10 || region_w < 3 || region_h < 3 {
                    continue;
                }

                let avg_score = score_sum / count as f32;
                if avg_score < threshold {
                    continue;
                }

                // Convert to image coordinates with slight expansion
                let bx = (min_x as f32 * scale_x).max(0.0);
                let by = (min_y as f32 * scale_y).max(0.0);
                let bw = ((region_w as f32) * scale_x).min(img_width as f32 - bx);
                let bh = ((region_h as f32) * scale_y).min(img_height as f32 - by);

                boxes.push(BoundingBox::new(bx, by, bw, bh));
            }
        }

        // Apply NMS to remove overlapping boxes
        self.non_max_suppression(boxes, 0.5)
    }

    /// Non-Maximum Suppression to remove overlapping boxes
    fn non_max_suppression(
        &self,
        mut boxes: Vec<BoundingBox>,
        iou_threshold: f32,
    ) -> Vec<BoundingBox> {
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
    #[allow(dead_code)]
    fn recognize_text(&self, input: Array4<f32>) -> Result<(String, f32), MlError> {
        let results = self.recognize_text_batch(&[input])?;
        Ok(results
            .into_iter()
            .next()
            .unwrap_or_else(|| (String::new(), 0.0)))
    }

    /// Recognize text in multiple regions with batched inference.
    /// Processes in sub-batches to handle varying widths efficiently.
    #[tracing::instrument(skip(self, inputs), fields(batch_size = %inputs.len()))]
    fn recognize_text_batch(&self, inputs: &[Array4<f32>]) -> Result<Vec<(String, f32)>, MlError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        // Process in sub-batches of max 8 to avoid memory issues
        // from padding many different widths to the same max width
        const MAX_BATCH: usize = 8;
        if inputs.len() > MAX_BATCH {
            let mut all_results = Vec::with_capacity(inputs.len());
            for chunk in inputs.chunks(MAX_BATCH) {
                all_results.extend(self.recognize_text_batch_inner(chunk)?);
            }
            return Ok(all_results);
        }

        self.recognize_text_batch_inner(inputs)
    }

    fn recognize_text_batch_inner(&self, inputs: &[Array4<f32>]) -> Result<Vec<(String, f32)>, MlError> {
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

        // Inputs may have different widths (text regions vary in length).
        // Pad all to the maximum width for batched inference.
        let channels = inputs[0].shape()[1];
        let height = inputs[0].shape()[2];
        let max_width = inputs.iter().map(|inp| inp.shape()[3]).max().unwrap_or(1);

        // Combine all inputs into a single batch tensor, zero-padding narrower inputs
        let mut batch_input = Array4::<f32>::zeros((batch_size, channels, height, max_width));
        for (i, input) in inputs.iter().enumerate() {
            let w = input.shape()[3];
            batch_input
                .slice_mut(ndarray::s![i, .., .., ..w])
                .assign(&input.slice(ndarray::s![0, .., .., ..]));
        }

        let input_tensor = TensorRef::from_array_view(&batch_input)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let outputs = recognition_session.run(ort::inputs!["x" => input_tensor])?;

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
            let model_vocab_size = shape[2] as usize;
            let sample_size = seq_len * model_vocab_size;

            for batch_idx in 0..batch_size {
                let start = batch_idx * sample_size;
                let end = start + sample_size;
                if end <= data.len() {
                    let sample_data = &data[start..end];
                    let (text, confidence) = self.decode_ctc_with_vocab_size(sample_data, model_vocab_size);
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

    /// Get the vocabulary/charset for CTC decoding.
    /// Must match the character dictionary used when training the ONNX model.
    fn get_vocabulary(&self) -> Vec<char> {
        self.config
            .decoder_config
            .vocabulary
            .clone()
            .unwrap_or_else(|| {
                // PaddleOCR PP-OCRv4 English recognition model character dictionary
                // (from ppocr/utils/en_dict.txt). 95 characters total.
                // Model output: Index 0 = blank/CTC, Index 1-95 = these chars, Index 96 = EOS
                // Order: digits, :;<=>?@, uppercase, [\]^_`, lowercase, {|}~, !"#$%&'()*+,-./, space
                "0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~!\"#$%&'()*+,-./ "
                    .chars()
                    .collect()
            })
    }

    /// Decode CTC output to text using configured strategy
    #[tracing::instrument(skip(self, logits), fields(logits_len = %logits.len(), strategy = ?self.config.decoder_config.strategy))]
    fn decode_ctc(&self, logits: &[f32]) -> (String, f32) {
        let vocabulary = self.get_vocabulary();
        let vocab_size = vocabulary.len() + 1; // +1 for blank token at index 0
        self.decode_ctc_with_vocab_size(logits, vocab_size)
    }

    /// Decode CTC output using the actual model output vocabulary size
    fn decode_ctc_with_vocab_size(&self, logits: &[f32], vocab_size: usize) -> (String, f32) {
        let vocabulary = self.get_vocabulary();
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

    /// Convert model output to log probabilities.
    /// PaddleOCR recognition model outputs softmax probabilities (output name: softmax_*.tmp_0),
    /// so we just take the log. For raw logit outputs, log-softmax would be needed instead.
    fn compute_log_probabilities(
        &self,
        probs: &[f32],
        vocab_size: usize,
        seq_len: usize,
    ) -> Vec<Vec<f32>> {
        let mut log_probs = Vec::with_capacity(seq_len);

        for t in 0..seq_len {
            let start = t * vocab_size;
            let end = (start + vocab_size).min(probs.len());
            let frame = &probs[start..end];

            // Model output is already softmax probabilities, just take log
            let frame_log_probs: Vec<f32> = frame
                .iter()
                .map(|&p| (p + 1e-10).ln()) // add epsilon to avoid log(0)
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

        for frame in log_probs.iter().take(seq_len) {
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

        for frame in log_probs.iter().take(seq_len) {
            let mut new_beams: std::collections::HashMap<String, (f32, f32)> =
                std::collections::HashMap::new();

            for (prefix, log_pb, log_pnb) in &beams {
                let log_p = self.log_add(*log_pb, *log_pnb);

                // Process blank
                let blank_log_prob = frame.get(blank_idx).copied().unwrap_or(f32::NEG_INFINITY);
                let entry = new_beams
                    .entry(prefix.clone())
                    .or_insert((f32::NEG_INFINITY, f32::NEG_INFINITY));
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
                            let entry = new_beams
                                .entry(new_prefix)
                                .or_insert((f32::NEG_INFINITY, f32::NEG_INFINITY));
                            entry.1 = self.log_add(entry.1, *log_pb + c_log_prob);

                            // Or stay with current prefix
                            let entry = new_beams
                                .entry(prefix.clone())
                                .or_insert((f32::NEG_INFINITY, f32::NEG_INFINITY));
                            entry.1 = self.log_add(entry.1, *log_pnb + c_log_prob);
                        } else {
                            // Different character: can always extend
                            let mut new_prefix = prefix.clone();
                            new_prefix.push(c);
                            let entry = new_beams
                                .entry(new_prefix)
                                .or_insert((f32::NEG_INFINITY, f32::NEG_INFINITY));
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
    fn sort_by_reading_order(&self, regions: &mut [TextRegion]) {
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
    #[tracing::instrument(skip(self, image), fields(width = %image.width(), height = %image.height()))]
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

        // Upscale small images so both detection and recognition get sufficient detail
        const MIN_SIDE: u32 = 960;
        let image = {
            let (w, h) = image.dimensions();
            if w.max(h) < MIN_SIDE {
                let scale = MIN_SIDE as f32 / w.max(h) as f32;
                let new_w = (w as f32 * scale) as u32;
                let new_h = (h as f32 * scale) as u32;
                tracing::debug!(
                    original_w = w, original_h = h,
                    new_w = new_w, new_h = new_h,
                    "Upscaling small image for OCR"
                );
                std::borrow::Cow::Owned(image.resize_exact(
                    new_w,
                    new_h,
                    image::imageops::FilterType::Lanczos3,
                ))
            } else {
                std::borrow::Cow::Borrowed(image)
            }
        };
        let image = image.as_ref();

        // Step 1: Preprocess for detection
        let detection_input = {
            let _span = tracing::info_span!("ocr_detection_preprocess").entered();
            self.preprocess_for_detection(image)?
        };

        // Step 2: Detect text regions
        let boxes = {
            let _span = tracing::info_span!("ocr_detection_inference").entered();
            self.detect_text_regions(image, detection_input)?
        };
        tracing::debug!(num_regions = %boxes.len(), "Detected text regions");

        if boxes.is_empty() {
            return Ok(OcrResult {
                text: String::new(),
                regions: Vec::new(),
                confidence: 0.0,
                processing_time_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Step 3: Preprocess all regions for batch recognition
        let recognition_inputs: Vec<Array4<f32>> = {
            let _span =
                tracing::info_span!("ocr_recognition_preprocess", num_regions = %boxes.len())
                    .entered();
            boxes
                .iter()
                .filter_map(|bbox| self.preprocess_for_recognition(image, bbox).ok())
                .collect()
        };

        // Step 4: Batch recognize all text regions in a single forward pass
        let recognition_results = {
            let _span = tracing::info_span!("ocr_recognition_inference", batch_size = %recognition_inputs.len()).entered();
            self.recognize_text_batch(&recognition_inputs)?
        };

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
