use thiserror::Error;

/// ML-related errors
#[derive(Error, Debug)]
pub enum MlError {
    #[error("Model loading error: {0}")]
    ModelLoading(String),

    #[error("Model not found at path: {0}")]
    ModelNotFound(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("Input preprocessing error: {0}")]
    Preprocessing(String),

    #[error("Output postprocessing error: {0}")]
    Postprocessing(String),

    #[error("Tokenization error: {0}")]
    Tokenization(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    #[error("ONNX Runtime error: {0}")]
    OnnxRuntime(String),

    #[error("OCR error: {0}")]
    Ocr(String),

    #[error("Configuration error: {0}")]
    Configuration(String),
}

impl From<ort::Error> for MlError {
    fn from(e: ort::Error) -> Self {
        MlError::OnnxRuntime(e.to_string())
    }
}

impl From<image::ImageError> for MlError {
    fn from(e: image::ImageError) -> Self {
        MlError::ImageProcessing(e.to_string())
    }
}
