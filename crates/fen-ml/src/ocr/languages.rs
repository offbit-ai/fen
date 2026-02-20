use super::types::Language;

/// Metadata for a language-specific recognition model on HuggingFace
pub struct LanguageModelInfo {
    /// HuggingFace file path for the ONNX model
    pub hf_model_path: &'static str,
    /// HuggingFace file path for the dictionary
    pub hf_dict_path: &'static str,
    /// Local output filename for the ONNX model
    pub local_model_name: &'static str,
    /// Local output filename for the dictionary
    pub local_dict_name: &'static str,
    /// Description for the download script
    pub description: &'static str,
}

/// Detection model is shared across all languages
pub const DETECTION_HF_PATH: &str = "det/ch_PP-OCRv4_det/model.onnx";
pub const DETECTION_LOCAL_NAME: &str = "ocr_detection.onnx";

impl Language {
    /// Get recognition model metadata for this language
    pub fn model_info(&self) -> LanguageModelInfo {
        match self {
            Language::English => LanguageModelInfo {
                hf_model_path: "rec/en_PP-OCRv4_rec/model.onnx",
                hf_dict_path: "rec/en_PP-OCRv4_rec/dict.txt",
                local_model_name: "ocr_rec_en.onnx",
                local_dict_name: "ocr_dicts/en_dict.txt",
                description: "PP-OCRv4 English recognition",
            },
            Language::Chinese => LanguageModelInfo {
                hf_model_path: "rec/ch_PP-OCRv4_rec/model.onnx",
                hf_dict_path: "rec/ch_PP-OCRv4_rec/dict.txt",
                local_model_name: "ocr_rec_ch.onnx",
                local_dict_name: "ocr_dicts/ch_dict.txt",
                description: "PP-OCRv4 Chinese recognition",
            },
            Language::Japanese => LanguageModelInfo {
                hf_model_path: "rec/japan_PP-OCRv3_rec/model.onnx",
                hf_dict_path: "rec/japan_PP-OCRv3_rec/dict.txt",
                local_model_name: "ocr_rec_ja.onnx",
                local_dict_name: "ocr_dicts/ja_dict.txt",
                description: "PP-OCRv3 Japanese recognition",
            },
            Language::Korean => LanguageModelInfo {
                hf_model_path: "rec/korean_PP-OCRv3_rec/model.onnx",
                hf_dict_path: "rec/korean_PP-OCRv3_rec/dict.txt",
                local_model_name: "ocr_rec_ko.onnx",
                local_dict_name: "ocr_dicts/ko_dict.txt",
                description: "PP-OCRv3 Korean recognition",
            },
            Language::Arabic => LanguageModelInfo {
                hf_model_path: "rec/arabic_PP-OCRv3_rec/model.onnx",
                hf_dict_path: "rec/arabic_PP-OCRv3_rec/dict.txt",
                local_model_name: "ocr_rec_ar.onnx",
                local_dict_name: "ocr_dicts/ar_dict.txt",
                description: "PP-OCRv3 Arabic recognition",
            },
            // French, German, Spanish all use the Latin model
            Language::French | Language::German | Language::Spanish => LanguageModelInfo {
                hf_model_path: "rec/latin_PP-OCRv3_rec/model.onnx",
                hf_dict_path: "rec/latin_PP-OCRv3_rec/dict.txt",
                local_model_name: "ocr_rec_latin.onnx",
                local_dict_name: "ocr_dicts/latin_dict.txt",
                description: "PP-OCRv3 Latin recognition (French/German/Spanish)",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_languages_have_model_info() {
        let languages = [
            Language::English,
            Language::Spanish,
            Language::French,
            Language::German,
            Language::Chinese,
            Language::Japanese,
            Language::Korean,
            Language::Arabic,
        ];

        for lang in &languages {
            let info = lang.model_info();
            assert!(!info.hf_model_path.is_empty());
            assert!(!info.hf_dict_path.is_empty());
            assert!(info.local_model_name.ends_with(".onnx"));
            assert!(info.local_dict_name.ends_with(".txt"));
        }
    }

    #[test]
    fn test_latin_languages_share_model() {
        let french = Language::French.model_info();
        let german = Language::German.model_info();
        let spanish = Language::Spanish.model_info();

        assert_eq!(french.local_model_name, german.local_model_name);
        assert_eq!(german.local_model_name, spanish.local_model_name);
        assert_eq!(french.local_model_name, "ocr_rec_latin.onnx");
    }
}
