use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use async_trait::async_trait;
use image::{DynamicImage, GenericImageView};
use ndarray::Array4;
use ort::session::Session;
use ort::value::TensorRef;

use super::{BoundingBox, OcrConfig, OcrResult, TextRegion};
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

    /// Recognize text in a region
    fn recognize_text(
        &self,
        input: Array4<f32>,
    ) -> Result<(String, f32), MlError> {
        let mut recognition_session = self
            .recognition_session
            .as_ref()
            .ok_or_else(|| MlError::ModelLoading("Recognition model not loaded".to_string()))?
            .lock()
            .map_err(|e| MlError::ModelLoading(format!("Failed to acquire session lock: {}", e)))?;

        let input_tensor = TensorRef::from_array_view(&input)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let outputs = recognition_session.run(ort::inputs!["input" => input_tensor])?;

        let output = if let Some(out) = outputs.get("output") {
            out
        } else {
            &outputs[0]
        };

        let (_, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| MlError::Postprocessing(e.to_string()))?;

        // Decode CTC output (simplified)
        let (text, confidence) = self.decode_ctc(data);

        Ok((text, confidence))
    }

    /// Decode CTC output to text
    fn decode_ctc(&self, logits: &[f32]) -> (String, f32) {
        // Simplified CTC decoding - in production, use proper vocabulary
        // This is a placeholder that would need the actual character set
        let charset: Vec<char> = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~".chars().collect();

        let mut text = String::new();
        let mut prev_idx: i32 = -1;
        let mut total_conf = 0.0;
        let mut count = 0;

        // Assume logits are shaped [seq_len, vocab_size]
        let vocab_size = charset.len() + 1; // +1 for blank token
        let seq_len = logits.len() / vocab_size;

        for t in 0..seq_len {
            let start = t * vocab_size;
            let end = start + vocab_size;
            if end > logits.len() {
                break;
            }

            let frame = &logits[start..end];

            // Softmax and argmax
            let max_val = frame.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            let exp_sum: f32 = frame.iter().map(|&x| (x - max_val).exp()).sum();

            let mut best_idx = 0;
            let mut best_prob = 0.0;
            for (i, &val) in frame.iter().enumerate() {
                let prob = (val - max_val).exp() / exp_sum;
                if prob > best_prob {
                    best_prob = prob;
                    best_idx = i;
                }
            }

            // Skip blank token (index 0) and repeated characters
            if best_idx != 0 && best_idx as i32 != prev_idx {
                if let Some(&c) = charset.get(best_idx - 1) {
                    text.push(c);
                    total_conf += best_prob;
                    count += 1;
                }
            }

            prev_idx = best_idx as i32;
        }

        let avg_conf = if count > 0 {
            total_conf / count as f32
        } else {
            0.0
        };

        (text, avg_conf)
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

        // Step 3: Recognize text in each region
        let mut regions = Vec::with_capacity(boxes.len());
        let mut total_confidence = 0.0;

        for (i, bbox) in boxes.iter().enumerate() {
            let recognition_input = self.preprocess_for_recognition(image, bbox)?;
            let (text, confidence) = self.recognize_text(recognition_input)?;

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
