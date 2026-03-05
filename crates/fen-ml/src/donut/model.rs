use std::path::Path;
use std::sync::Mutex;

use image::DynamicImage;
use ndarray::Array4;
use ort::session::Session;
use ort::value::TensorRef;
use serde::{Deserialize, Serialize};
use tokenizers::Tokenizer;

use super::decoder;
use crate::error::MlError;

/// Configuration for the Donut vision encoder-decoder model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonutModelConfig {
    /// Path to encoder ONNX model file
    pub encoder_path: Option<String>,

    /// Path to decoder ONNX model file
    pub decoder_path: Option<String>,

    /// Path to tokenizer.json
    pub tokenizer_path: Option<String>,

    /// Maximum number of tokens to generate
    pub max_decode_length: usize,

    /// Input image height (Donut default: 2560 → typically resized)
    pub image_height: u32,

    /// Input image width (Donut default: 1920 → typically resized)
    pub image_width: u32,
}

impl Default for DonutModelConfig {
    fn default() -> Self {
        Self {
            encoder_path: None,
            decoder_path: None,
            tokenizer_path: None,
            max_decode_length: 1024,
            image_height: 2560,
            image_width: 1920,
        }
    }
}

/// Output from Donut processing
#[derive(Debug, Clone)]
pub struct DonutOutput {
    /// Raw decoded text from the model
    pub raw_text: String,

    /// Parsed JSON output (if valid JSON was decoded)
    pub json: Option<serde_json::Value>,
}

/// Donut vision encoder-decoder model for end-to-end document understanding.
///
/// Processes document images directly to structured JSON without OCR.
/// Uses Swin Transformer encoder + BART decoder with autoregressive decoding.
pub struct DonutModel {
    config: DonutModelConfig,
    encoder: Option<Mutex<Session>>,
    decoder: Option<Mutex<Session>>,
    tokenizer: Option<Tokenizer>,
}

impl DonutModel {
    /// Create model without ONNX sessions (returns empty results)
    pub fn new(config: DonutModelConfig) -> Result<Self, MlError> {
        Ok(Self {
            config,
            encoder: None,
            decoder: None,
            tokenizer: None,
        })
    }

    /// Create model with ONNX encoder + decoder sessions
    pub fn with_models(
        config: DonutModelConfig,
        models_dir: impl AsRef<Path>,
    ) -> Result<Self, MlError> {
        let models_dir = models_dir.as_ref();

        let (encoder_path, decoder_path, tokenizer_path) = match (
            &config.encoder_path,
            &config.decoder_path,
            &config.tokenizer_path,
        ) {
            (Some(enc), Some(dec), Some(tok)) => (
                models_dir.join(enc),
                models_dir.join(dec),
                models_dir.join(tok),
            ),
            _ => return Self::new(config),
        };

        if !encoder_path.exists() {
            return Err(MlError::ModelNotFound(encoder_path.display().to_string()));
        }
        if !decoder_path.exists() {
            return Err(MlError::ModelNotFound(decoder_path.display().to_string()));
        }
        if !tokenizer_path.exists() {
            return Err(MlError::ModelNotFound(tokenizer_path.display().to_string()));
        }

        let encoder_session = Session::builder()?.commit_from_file(&encoder_path)?;
        let decoder_session = Session::builder()?.commit_from_file(&decoder_path)?;

        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| MlError::Tokenization(e.to_string()))?;

        tracing::info!(
            encoder = %encoder_path.display(),
            decoder = %decoder_path.display(),
            tokenizer = %tokenizer_path.display(),
            "Loaded Donut model"
        );

        Ok(Self {
            config,
            encoder: Some(Mutex::new(encoder_session)),
            decoder: Some(Mutex::new(decoder_session)),
            tokenizer: Some(tokenizer),
        })
    }

    /// Check if model is loaded
    pub fn has_model(&self) -> bool {
        self.encoder.is_some() && self.decoder.is_some() && self.tokenizer.is_some()
    }

    /// Process a document image and return structured output.
    ///
    /// Uses the CORD-v2 task prompt (`<s_cord-v2>`) by default.
    pub fn process(
        &self,
        image: &DynamicImage,
        task_prompt: Option<&str>,
    ) -> Result<DonutOutput, MlError> {
        if !self.has_model() {
            return Err(MlError::ModelLoading(
                "Donut model not loaded".to_string(),
            ));
        }

        let tokenizer = self.tokenizer.as_ref().unwrap();
        let encoder = self.encoder.as_ref().unwrap();
        let decoder_session = self.decoder.as_ref().unwrap();

        // Step 1: Preprocess image → [1, 3, H, W] normalized tensor
        let pixel_values = self.preprocess_image(image)?;

        // Step 2: Encode image
        let encoder_hidden_states = self.encode(&pixel_values, encoder)?;

        // Step 3: Tokenize task prompt
        let prompt = task_prompt.unwrap_or("<s_cord-v2>");
        let prompt_encoding = tokenizer
            .encode(prompt, false)
            .map_err(|e| MlError::Tokenization(e.to_string()))?;
        let prompt_ids: Vec<i64> = prompt_encoding
            .get_ids()
            .iter()
            .map(|&id| id as i64)
            .collect();

        // Determine EOS token ID
        let eos_token_id = tokenizer
            .token_to_id("</s>")
            .unwrap_or(2) as i64;

        // Step 4: Autoregressive decode
        let generated_ids = decoder::autoregressive_decode(
            decoder_session,
            &encoder_hidden_states,
            &prompt_ids,
            eos_token_id,
            self.config.max_decode_length,
        )?;

        // Step 5: Decode tokens to text
        let token_ids_u32: Vec<u32> = generated_ids.iter().map(|&id| id as u32).collect();
        let raw_text = tokenizer
            .decode(&token_ids_u32, true)
            .map_err(|e| MlError::Tokenization(e.to_string()))?;

        // Step 6: Try to parse as JSON
        let json = extract_json(&raw_text);

        Ok(DonutOutput { raw_text, json })
    }

    /// Preprocess image: resize and normalize to [1, 3, H, W] tensor.
    fn preprocess_image(&self, image: &DynamicImage) -> Result<Array4<f32>, MlError> {
        let resized = image.resize_exact(
            self.config.image_width,
            self.config.image_height,
            image::imageops::FilterType::Lanczos3,
        );

        let rgb = resized.to_rgb8();
        let (w, h) = (rgb.width() as usize, rgb.height() as usize);

        // ImageNet normalization
        let mean = [0.485f32, 0.456, 0.406];
        let std = [0.229f32, 0.224, 0.225];

        let mut data = vec![0.0f32; 3 * h * w];
        for y in 0..h {
            for x in 0..w {
                let pixel = rgb.get_pixel(x as u32, y as u32);
                for c in 0..3 {
                    let val = pixel[c] as f32 / 255.0;
                    data[c * h * w + y * w + x] = (val - mean[c]) / std[c];
                }
            }
        }

        Array4::from_shape_vec((1, 3, h, w), data)
            .map_err(|e| MlError::Preprocessing(e.to_string()))
    }

    /// Run encoder and return hidden states.
    fn encode(
        &self,
        pixel_values: &Array4<f32>,
        encoder: &Mutex<Session>,
    ) -> Result<ndarray::Array3<f32>, MlError> {
        let pixel_t = TensorRef::from_array_view(pixel_values)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let mut session = encoder.lock().map_err(|e| {
            MlError::ModelLoading(format!("Failed to acquire encoder lock: {}", e))
        })?;

        let outputs = session.run(ort::inputs![
            "pixel_values" => pixel_t
        ])?;

        let output = if let Some(out) = outputs.get("last_hidden_state") {
            out
        } else {
            &outputs[0]
        };

        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| MlError::Postprocessing(e.to_string()))?;

        // shape: [batch, seq, hidden_dim]
        let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
        if dims.len() != 3 {
            return Err(MlError::Postprocessing(format!(
                "Expected 3D encoder output, got {}D",
                dims.len()
            )));
        }

        ndarray::Array3::from_shape_vec((dims[0], dims[1], dims[2]), data.to_vec())
            .map_err(|e| MlError::Postprocessing(e.to_string()))
    }
}

/// Extract JSON from Donut's decoded text output.
///
/// Donut outputs text that may contain XML-like tags mixed with JSON.
/// This function finds the outermost `{...}` and parses it.
fn extract_json(text: &str) -> Option<serde_json::Value> {
    // Find first '{' and last '}'
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }

    let json_str = &text[start..=end];
    serde_json::from_str(json_str).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_donut_model_without_onnx() {
        let config = DonutModelConfig::default();
        let model = DonutModel::new(config).unwrap();
        assert!(!model.has_model());
    }

    #[test]
    fn test_default_config() {
        let config = DonutModelConfig::default();
        assert_eq!(config.max_decode_length, 1024);
        assert_eq!(config.image_height, 2560);
        assert_eq!(config.image_width, 1920);
    }

    #[test]
    fn test_extract_json() {
        assert_eq!(
            extract_json(r#"<s_cord-v2>{"total": "10.00"}</s>"#),
            Some(serde_json::json!({"total": "10.00"}))
        );
        assert_eq!(extract_json("no json here"), None);
        assert_eq!(extract_json(""), None);
    }

    #[test]
    fn test_extract_json_nested() {
        let text = r#"output: {"menu": [{"nm": "coffee", "price": "3.50"}], "total": {"total_price": "3.50"}}"#;
        let json = extract_json(text).unwrap();
        assert!(json["menu"].is_array());
        assert_eq!(json["total"]["total_price"], "3.50");
    }
}
