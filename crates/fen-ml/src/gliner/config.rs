use serde::{Deserialize, Serialize};

/// Configuration for a single GLiNER model tier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlinerModelConfig {
    /// Path to ONNX model file
    pub model_path: Option<String>,

    /// Path to fine-tuned ONNX model (overrides model_path when present)
    pub finetuned_model_path: Option<String>,

    /// Path to tokenizer.json
    pub tokenizer_path: Option<String>,

    /// Maximum sequence length (tokens)
    pub max_seq_length: usize,

    /// Maximum span width (in words) to enumerate
    pub max_span_width: usize,

    /// Minimum score threshold after sigmoid
    pub threshold: f32,

    /// Model name (for logging)
    pub model_name: String,
}

impl GlinerModelConfig {
    pub fn medium() -> Self {
        Self {
            model_path: None,
            finetuned_model_path: None,
            tokenizer_path: None,
            max_seq_length: 384,
            max_span_width: 12,
            threshold: 0.5,
            model_name: "gliner-medium-v2.1".to_string(),
        }
    }

    pub fn large() -> Self {
        Self {
            model_path: None,
            finetuned_model_path: None,
            tokenizer_path: None,
            max_seq_length: 512,
            max_span_width: 12,
            threshold: 0.4,
            model_name: "gliner-large-v2.1".to_string(),
        }
    }

    /// Returns the effective model path, preferring fine-tuned over base.
    pub fn effective_model_path(&self) -> Option<&str> {
        self.finetuned_model_path
            .as_deref()
            .or(self.model_path.as_deref())
    }
}

impl Default for GlinerModelConfig {
    fn default() -> Self {
        Self::medium()
    }
}

/// Configuration for the tiered GLiNER extraction system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlinerConfig {
    /// Medium model configuration (first tier)
    pub medium: GlinerModelConfig,

    /// Large model configuration (second tier, escalation)
    pub large: GlinerModelConfig,

    /// Overall confidence threshold for escalation
    pub escalation_threshold: f32,

    /// Minimum completeness ratio before escalation
    pub completeness_threshold: f32,

    /// NMS IoU threshold for overlapping span suppression
    pub nms_threshold: f32,
}

impl Default for GlinerConfig {
    fn default() -> Self {
        Self {
            medium: GlinerModelConfig::medium(),
            large: GlinerModelConfig::large(),
            escalation_threshold: 0.65,
            completeness_threshold: 0.6,
            nms_threshold: 0.5,
        }
    }
}
