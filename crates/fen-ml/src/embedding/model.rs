use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Mutex;

use lru::LruCache;
use ndarray::Array2;
use ort::session::Session;
use ort::value::TensorRef;
use tokenizers::{PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

use super::{
    DocumentEmbeddings, EmbeddingModelConfig, EntityEmbedding, SectionEmbedding, SimilarityResult,
};
use crate::error::MlError;

/// Default cache size for embeddings (1024 entries)
const DEFAULT_CACHE_SIZE: usize = 1024;

/// Sentence transformer style embedding model with proper batch processing
/// Includes an LRU cache to avoid recomputing embeddings for repeated texts
pub struct EmbeddingModel {
    config: EmbeddingModelConfig,
    session: Option<Mutex<Session>>,
    tokenizer: Option<Tokenizer>,
    /// LRU cache for embeddings, keyed by text hash
    cache: Mutex<LruCache<u64, Vec<f32>>>,
}

impl EmbeddingModel {
    /// Create embedding model without ONNX model (returns zero embeddings)
    pub fn new(config: EmbeddingModelConfig) -> Result<Self, MlError> {
        let cache_size = NonZeroUsize::new(DEFAULT_CACHE_SIZE).unwrap();
        Ok(Self {
            config,
            session: None,
            tokenizer: None,
            cache: Mutex::new(LruCache::new(cache_size)),
        })
    }

    /// Create embedding model with ONNX model and tokenizer
    pub fn with_model(
        config: EmbeddingModelConfig,
        model_path: impl AsRef<Path>,
        tokenizer_path: impl AsRef<Path>,
    ) -> Result<Self, MlError> {
        let model_path = model_path.as_ref();
        let tokenizer_path = tokenizer_path.as_ref();

        if !model_path.exists() {
            return Err(MlError::ModelNotFound(model_path.display().to_string()));
        }
        if !tokenizer_path.exists() {
            return Err(MlError::ModelNotFound(
                tokenizer_path.display().to_string(),
            ));
        }

        let session = Session::builder()?.commit_from_file(model_path)?;

        let mut tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| MlError::Tokenization(e.to_string()))?;

        // Configure tokenizer for batching with padding and truncation
        tokenizer
            .with_padding(Some(PaddingParams {
                strategy: PaddingStrategy::BatchLongest,
                ..Default::default()
            }))
            .with_truncation(Some(TruncationParams {
                max_length: config.max_seq_length,
                ..Default::default()
            }))
            .map_err(|e| MlError::Tokenization(e.to_string()))?;

        tracing::info!(
            model = %model_path.display(),
            tokenizer = %tokenizer_path.display(),
            "Loaded embedding model"
        );

        let cache_size = NonZeroUsize::new(DEFAULT_CACHE_SIZE).unwrap();
        Ok(Self {
            config,
            session: Some(Mutex::new(session)),
            tokenizer: Some(tokenizer),
            cache: Mutex::new(LruCache::new(cache_size)),
        })
    }

    /// Check if model is loaded
    pub fn has_model(&self) -> bool {
        self.session.is_some() && self.tokenizer.is_some()
    }

    /// Embedding dimension
    pub fn embedding_dim(&self) -> usize {
        self.config.embedding_dim
    }

    /// Generate embedding for a single text
    /// Uses LRU cache to avoid recomputing embeddings for repeated texts
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, MlError> {
        if !self.has_model() {
            return Ok(vec![0.0; self.config.embedding_dim]);
        }

        // Compute hash of text for cache key
        let text_hash = self.hash_text(text);

        // Check cache first
        if let Ok(mut cache) = self.cache.lock() {
            if let Some(cached) = cache.get(&text_hash) {
                return Ok(cached.clone());
            }
        }

        // Compute embedding
        let embeddings = self.embed_batch(&[text])?;
        let embedding = embeddings
            .into_iter()
            .next()
            .unwrap_or_else(|| vec![0.0; self.config.embedding_dim]);

        // Store in cache
        if let Ok(mut cache) = self.cache.lock() {
            cache.put(text_hash, embedding.clone());
        }

        Ok(embedding)
    }

    /// Compute a hash of the text for cache lookup
    fn hash_text(&self, text: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }

    /// Generate embeddings for multiple texts with efficient batching
    ///
    /// This processes all texts in a single forward pass for better performance
    /// compared to processing one at a time.
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, MlError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        if !self.has_model() {
            return Ok(texts
                .iter()
                .map(|_| vec![0.0; self.config.embedding_dim])
                .collect());
        }

        let mut session = self
            .session
            .as_ref()
            .unwrap()
            .lock()
            .map_err(|e| MlError::ModelLoading(format!("Failed to acquire session lock: {}", e)))?;
        let tokenizer = self.tokenizer.as_ref().unwrap();

        let batch_size = texts.len();

        // Tokenize all texts at once - tokenizers crate handles padding automatically
        let encodings = tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| MlError::Tokenization(e.to_string()))?;

        // Find max sequence length in this batch (after padding)
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0)
            .min(self.config.max_seq_length);

        if max_len == 0 {
            return Ok(texts
                .iter()
                .map(|_| vec![0.0; self.config.embedding_dim])
                .collect());
        }

        // Build input tensors with proper batch dimensions
        let mut input_ids = Array2::<i64>::zeros((batch_size, max_len));
        let mut attention_mask = Array2::<i64>::zeros((batch_size, max_len));

        for (batch_idx, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();

            for (seq_idx, (&id, &m)) in ids.iter().zip(mask.iter()).take(max_len).enumerate() {
                input_ids[[batch_idx, seq_idx]] = id as i64;
                attention_mask[[batch_idx, seq_idx]] = m as i64;
            }
        }

        // Create tensor references from arrays
        let input_ids_tensor = TensorRef::from_array_view(&input_ids)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let attention_mask_tensor = TensorRef::from_array_view(&attention_mask)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        // Run batched inference
        let outputs = session.run(ort::inputs![
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor
        ])?;

        // Get last hidden state output - try by name first, otherwise use first output
        let output = if let Some(out) = outputs.get("last_hidden_state") {
            out
        } else {
            &outputs[0]
        };

        let (shape, data) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| MlError::Postprocessing(e.to_string()))?;

        // Shape derefs to [i64] slice
        if shape.len() != 3 {
            return Err(MlError::Postprocessing(format!(
                "Expected 3D output tensor [batch, seq, hidden], got {}D",
                shape.len()
            )));
        }

        let seq_len_out = shape[1] as usize;
        let hidden_size = shape[2] as usize;
        let mut embeddings = Vec::with_capacity(batch_size);

        // Mean pooling for each item in batch
        for batch_idx in 0..batch_size {
            let encoding = &encodings[batch_idx];
            let mask = encoding.get_attention_mask();
            let mask_len = mask.len().min(seq_len_out);

            let mut sum = vec![0.0f32; hidden_size];
            let mut count = 0.0f32;

            for i in 0..mask_len {
                if i < mask.len() && mask[i] == 1 {
                    let offset = batch_idx * seq_len_out * hidden_size + i * hidden_size;
                    for j in 0..hidden_size {
                        if offset + j < data.len() {
                            sum[j] += data[offset + j];
                        }
                    }
                    count += 1.0;
                }
            }

            // Compute mean
            if count > 0.0 {
                for x in &mut sum {
                    *x /= count;
                }
            }

            // L2 normalize
            let normalized = self.normalize(&sum);
            embeddings.push(normalized);
        }

        Ok(embeddings)
    }

    /// Generate document embeddings at multiple granularities
    ///
    /// Uses batch processing internally for efficiency
    pub fn embed_document(
        &self,
        full_text: &str,
        sections: &[(String, String, String)], // (id, type, text)
        entities: &[(String, String)],          // (type, value)
    ) -> Result<DocumentEmbeddings, MlError> {
        // Collect all texts for single batch processing
        let mut all_texts: Vec<&str> = Vec::with_capacity(1 + sections.len() + entities.len());
        all_texts.push(full_text);
        for (_, _, text) in sections {
            all_texts.push(text.as_str());
        }
        for (_, value) in entities {
            all_texts.push(value.as_str());
        }

        // Batch embed all texts in one forward pass
        let all_embeddings = self.embed_batch(&all_texts)?;

        let mut idx = 0;

        // Full document embedding
        let document_embedding = all_embeddings
            .get(idx)
            .cloned()
            .unwrap_or_else(|| vec![0.0; self.config.embedding_dim]);
        idx += 1;

        // Section embeddings
        let section_embeddings: Vec<SectionEmbedding> = sections
            .iter()
            .map(|(id, section_type, text)| {
                let embedding = all_embeddings
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| vec![0.0; self.config.embedding_dim]);
                idx += 1;
                SectionEmbedding {
                    section_id: id.clone(),
                    section_type: section_type.clone(),
                    embedding,
                    text: text.clone(),
                }
            })
            .collect();

        // Entity embeddings
        let entity_embeddings: Vec<EntityEmbedding> = entities
            .iter()
            .map(|(entity_type, value)| {
                let embedding = all_embeddings
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| vec![0.0; self.config.embedding_dim]);
                idx += 1;
                EntityEmbedding {
                    entity_type: entity_type.clone(),
                    value: value.clone(),
                    embedding,
                }
            })
            .collect();

        Ok(DocumentEmbeddings {
            document: document_embedding,
            sections: section_embeddings,
            entities: entity_embeddings,
        })
    }

    /// Compute cosine similarity between two embeddings
    pub fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Find most similar embeddings from candidates
    pub fn find_similar(
        &self,
        query: &[f32],
        candidates: &[Vec<f32>],
        top_k: usize,
    ) -> Vec<SimilarityResult> {
        let mut results: Vec<SimilarityResult> = candidates
            .iter()
            .enumerate()
            .map(|(i, candidate)| {
                let score = self.cosine_similarity(query, candidate);
                SimilarityResult::new(i, score)
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        results.truncate(top_k);
        results
    }

    /// L2 normalize embedding vector
    fn normalize(&self, embedding: &[f32]) -> Vec<f32> {
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm > 0.0 {
            embedding.iter().map(|x| x / norm).collect()
        } else {
            embedding.to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_model_without_onnx() {
        let config = EmbeddingModelConfig::default();
        let model = EmbeddingModel::new(config).unwrap();
        assert!(!model.has_model());

        let embedding = model.embed("test text").unwrap();
        assert_eq!(embedding.len(), 768);
        assert!(embedding.iter().all(|&x| x == 0.0));
    }

    #[test]
    fn test_batch_embedding_without_model() {
        let config = EmbeddingModelConfig::default();
        let model = EmbeddingModel::new(config).unwrap();

        let texts = ["hello world", "test sentence", "another one"];
        let embeddings = model.embed_batch(&texts).unwrap();

        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), 768);
        }
    }

    #[test]
    fn test_cosine_similarity() {
        let config = EmbeddingModelConfig::default();
        let model = EmbeddingModel::new(config).unwrap();

        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((model.cosine_similarity(&a, &b) - 1.0).abs() < 0.0001);

        let c = vec![0.0, 1.0, 0.0];
        assert!(model.cosine_similarity(&a, &c).abs() < 0.0001);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((model.cosine_similarity(&a, &d) + 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_find_similar() {
        let config = EmbeddingModelConfig::default();
        let model = EmbeddingModel::new(config).unwrap();

        let query = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            vec![0.5, 0.5, 0.0],
            vec![0.9, 0.1, 0.0],
            vec![0.0, 1.0, 0.0],
        ];

        let results = model.find_similar(&query, &candidates, 2);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].index, 1); // Most similar
    }

    #[test]
    fn test_embed_document() {
        let config = EmbeddingModelConfig::default();
        let model = EmbeddingModel::new(config).unwrap();

        let sections = vec![
            ("s1".to_string(), "header".to_string(), "Header text".to_string()),
            ("s2".to_string(), "body".to_string(), "Body content".to_string()),
        ];
        let entities = vec![
            ("amount".to_string(), "$100.00".to_string()),
        ];

        let result = model.embed_document("Full document", &sections, &entities).unwrap();
        assert_eq!(result.document.len(), 768);
        assert_eq!(result.sections.len(), 2);
        assert_eq!(result.entities.len(), 1);
    }
}
