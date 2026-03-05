//! Autoregressive token-by-token decoding for Donut's BART decoder.

use std::sync::Mutex;

use ndarray::{Array2, Array3};
use ort::session::Session;
use ort::value::TensorRef;

use crate::error::MlError;

/// Run autoregressive decoding using the decoder session and encoder hidden states.
///
/// Returns the sequence of generated token IDs (excluding the initial prompt tokens).
pub fn autoregressive_decode(
    decoder: &Mutex<Session>,
    encoder_hidden_states: &Array3<f32>,
    prompt_token_ids: &[i64],
    eos_token_id: i64,
    max_length: usize,
) -> Result<Vec<i64>, MlError> {
    let mut generated = prompt_token_ids.to_vec();

    for _ in 0..max_length {
        let seq_len = generated.len();

        // Build decoder input tensors
        let decoder_input_ids =
            Array2::from_shape_vec((1, seq_len), generated.clone())
                .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let decoder_attention_mask =
            Array2::from_shape_vec((1, seq_len), vec![1i64; seq_len])
                .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let decoder_input_ids_t = TensorRef::from_array_view(&decoder_input_ids)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let decoder_attention_mask_t = TensorRef::from_array_view(&decoder_attention_mask)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;
        let encoder_hidden_states_t = TensorRef::from_array_view(encoder_hidden_states)
            .map_err(|e| MlError::Preprocessing(e.to_string()))?;

        let mut session = decoder.lock().map_err(|e| {
            MlError::ModelLoading(format!("Failed to acquire decoder lock: {}", e))
        })?;

        let outputs = session.run(ort::inputs![
            "input_ids" => decoder_input_ids_t,
            "attention_mask" => decoder_attention_mask_t,
            "encoder_hidden_states" => encoder_hidden_states_t
        ])?;

        // Extract logits: [batch, seq_len, vocab_size]
        let output = if let Some(out) = outputs.get("logits") {
            out
        } else {
            &outputs[0]
        };

        let (_, logits) = output
            .try_extract_tensor::<f32>()
            .map_err(|e| MlError::Postprocessing(e.to_string()))?;

        // logits shape: [1, seq_len, vocab_size]
        let logits_vec = logits.to_vec();
        let vocab_size = logits_vec.len() / seq_len;
        let last_token_logits = &logits_vec[(seq_len - 1) * vocab_size..seq_len * vocab_size];

        // Greedy: take argmax of last position
        let next_token = last_token_logits
            .iter()
            .enumerate()
            .max_by(|a: &(usize, &f32), b: &(usize, &f32)| {
                a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(idx, _)| idx as i64)
            .unwrap_or(eos_token_id);

        if next_token == eos_token_id {
            break;
        }

        generated.push(next_token);
    }

    // Return only the generated tokens (exclude prompt)
    Ok(generated[prompt_token_ids.len()..].to_vec())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_decoder_module_exists() {
        // Smoke test — decoder logic requires ONNX sessions to test fully
    }
}
