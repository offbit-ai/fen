use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::preprocessing::PreprocessingConfig;

/// Configuration for OCR engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    /// Path to detection model (ONNX)
    pub detection_model_path: Option<String>,

    /// Path to recognition model (ONNX)
    pub recognition_model_path: Option<String>,

    /// Path to vocabulary/dictionary file (relative to models_dir)
    pub vocabulary_path: Option<String>,

    /// Per-language recognition model overrides (language → model + dict paths)
    pub language_model_paths: HashMap<Language, LanguageModelPaths>,

    /// Languages to detect
    pub languages: Vec<Language>,

    /// Confidence threshold for character recognition
    pub confidence_threshold: f32,

    /// Detection threshold for text region binarization (lower = more sensitive)
    pub detection_threshold: f32,

    /// Maximum image dimension (resize larger images)
    pub max_dimension: u32,

    /// Enable GPU acceleration
    pub gpu_enabled: bool,

    /// CTC decoder configuration
    pub decoder_config: CtcDecoderConfig,

    /// Image preprocessing configuration (applied before detection)
    pub preprocessing: PreprocessingConfig,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            detection_model_path: None,
            recognition_model_path: None,
            vocabulary_path: None,
            language_model_paths: HashMap::new(),
            languages: vec![Language::English],
            confidence_threshold: 0.5,
            detection_threshold: 0.3,
            max_dimension: 2048,
            gpu_enabled: false,
            decoder_config: CtcDecoderConfig::default(),
            preprocessing: PreprocessingConfig::default(),
        }
    }
}

/// Paths for a language-specific recognition model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageModelPaths {
    /// Path to recognition ONNX model (relative to models_dir)
    pub model_path: String,
    /// Path to character dictionary file (relative to models_dir)
    pub dict_path: String,
}

/// CTC decoder configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtcDecoderConfig {
    /// Decoding strategy
    pub strategy: CtcDecodingStrategy,

    /// Beam width for beam search (ignored for greedy)
    pub beam_width: usize,

    /// Blank token index (usually 0)
    pub blank_index: usize,

    /// Custom vocabulary (if None, uses default ASCII + common symbols)
    pub vocabulary: Option<Vec<char>>,
}

impl Default for CtcDecoderConfig {
    fn default() -> Self {
        Self {
            strategy: CtcDecodingStrategy::BeamSearch,
            beam_width: 10,
            blank_index: 0,
            vocabulary: None,
        }
    }
}

/// CTC decoding strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CtcDecodingStrategy {
    /// Simple greedy decoding (fastest, less accurate)
    Greedy,
    /// Beam search decoding (slower, more accurate)
    BeamSearch,
}

/// Supported languages for OCR
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    English,
    Spanish,
    French,
    German,
    Chinese,
    Japanese,
    Korean,
    Arabic,
}

/// Result of OCR processing on an image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    /// Full extracted text
    pub text: String,

    /// Individual text regions detected
    pub regions: Vec<TextRegion>,

    /// Overall confidence score (0.0 - 1.0)
    pub confidence: f32,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

impl Default for OcrResult {
    fn default() -> Self {
        Self {
            text: String::new(),
            regions: Vec::new(),
            confidence: 0.0,
            processing_time_ms: 0,
        }
    }
}

/// A detected text region in an image
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRegion {
    /// Extracted text from this region
    pub text: String,

    /// Bounding box [x, y, width, height] in pixels
    pub bbox: BoundingBox,

    /// Confidence score for this region (0.0 - 1.0)
    pub confidence: f32,

    /// Reading order index
    pub order: usize,
}

/// Bounding box for a region
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl BoundingBox {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Calculate area of bounding box
    pub fn area(&self) -> f32 {
        self.width * self.height
    }

    /// Check if this box intersects with another
    pub fn intersects(&self, other: &BoundingBox) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    /// Calculate IoU (Intersection over Union) with another box
    pub fn iou(&self, other: &BoundingBox) -> f32 {
        if !self.intersects(other) {
            return 0.0;
        }

        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);

        let intersection = (x2 - x1) * (y2 - y1);
        let union = self.area() + other.area() - intersection;

        if union > 0.0 {
            intersection / union
        } else {
            0.0
        }
    }
}

/// OCR word with position information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrWord {
    pub text: String,
    pub bbox: BoundingBox,
    pub confidence: f32,
}

/// OCR line containing multiple words
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrLine {
    pub words: Vec<OcrWord>,
    pub bbox: BoundingBox,
    pub text: String,
}

impl OcrLine {
    pub fn from_words(words: Vec<OcrWord>) -> Self {
        let text = words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let bbox = if words.is_empty() {
            BoundingBox::new(0.0, 0.0, 0.0, 0.0)
        } else {
            let min_x = words.iter().map(|w| w.bbox.x).fold(f32::MAX, f32::min);
            let min_y = words.iter().map(|w| w.bbox.y).fold(f32::MAX, f32::min);
            let max_x = words
                .iter()
                .map(|w| w.bbox.x + w.bbox.width)
                .fold(f32::MIN, f32::max);
            let max_y = words
                .iter()
                .map(|w| w.bbox.y + w.bbox.height)
                .fold(f32::MIN, f32::max);

            BoundingBox::new(min_x, min_y, max_x - min_x, max_y - min_y)
        };

        Self { words, bbox, text }
    }
}
