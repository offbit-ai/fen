use tokenizers::Tokenizer;

use crate::error::MlError;

/// Word info tracking character offsets
#[derive(Debug, Clone)]
pub struct WordInfo {
    pub text: String,
    pub char_start: usize,
    pub char_end: usize,
}

/// Preprocessed input ready for GLiNER ONNX model
#[derive(Debug, Clone)]
pub struct GlinerInput {
    /// Token IDs: [CLS] label1 [SEP] label2 [SEP] ... [SEP] word1 word2 ... [SEP]
    pub input_ids: Vec<i64>,

    /// Attention mask (1 for real tokens, 0 for padding)
    pub attention_mask: Vec<i64>,

    /// Token type IDs (0 for labels, 1 for text)
    pub token_type_ids: Vec<i64>,

    /// Words mask: 1 at the first subword token of each text word, 0 elsewhere
    pub words_mask: Vec<i64>,

    /// Text lengths: number of text words
    pub text_lengths: Vec<i64>,

    /// Span indices: [num_spans, 2] (start_word_idx, end_word_idx)
    pub span_idx: Vec<[i64; 2]>,

    /// Span mask: 1 for valid spans, 0 for padding
    pub span_mask: Vec<i64>,

    /// Number of labels
    pub num_labels: usize,

    /// Sequence length after padding
    pub seq_length: usize,

    /// Word infos for mapping back to character offsets
    pub words: Vec<WordInfo>,
}

/// Split text into words, tracking character offsets
pub fn split_into_words(text: &str) -> Vec<WordInfo> {
    let mut words = Vec::new();
    let mut chars = text.char_indices().peekable();

    while let Some(&(start, ch)) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        let mut end = start;
        while let Some(&(i, c)) = chars.peek() {
            if c.is_whitespace() {
                break;
            }
            end = i + c.len_utf8();
            chars.next();
        }

        words.push(WordInfo {
            text: text[start..end].to_string(),
            char_start: start,
            char_end: end,
        });
    }

    words
}

/// Enumerate all candidate spans up to max_width
///
/// Returns pairs of (start_word_idx, end_word_idx) where end is exclusive.
/// For N words and max_width W, produces O(N * W) spans.
pub fn enumerate_spans(num_words: usize, max_span_width: usize) -> Vec<[usize; 2]> {
    let mut spans = Vec::with_capacity(num_words * max_span_width);

    for start in 0..num_words {
        let max_end = (start + max_span_width).min(num_words);
        for end in (start + 1)..=max_end {
            spans.push([start, end]);
        }
    }

    spans
}

/// Preprocess text and labels into GLiNER model inputs
pub fn preprocess(
    text: &str,
    labels: &[&str],
    tokenizer: &Tokenizer,
    max_seq_length: usize,
    max_span_width: usize,
) -> Result<GlinerInput, MlError> {
    let words = split_into_words(text);
    let num_labels = labels.len();

    // Build the combined sequence:
    // [CLS] label1 [SEP] label2 [SEP] ... [SEP] word1 word2 ... [SEP]
    //
    // GLiNER uses a special tokenization where labels come first,
    // then the text tokens follow.

    // Tokenize labels to compute label embeddings positions
    let mut all_tokens: Vec<u32> = Vec::new();
    let mut token_type_ids_raw: Vec<i64> = Vec::new();

    // [CLS] token
    let cls_id = get_special_token_id(tokenizer, "[CLS]").unwrap_or(101);
    let sep_id = get_special_token_id(tokenizer, "[SEP]").unwrap_or(102);

    all_tokens.push(cls_id);
    token_type_ids_raw.push(0); // label segment

    // Encode each label
    for label in labels {
        let encoding = tokenizer
            .encode(*label, false)
            .map_err(|e| MlError::Tokenization(e.to_string()))?;
        for &id in encoding.get_ids() {
            all_tokens.push(id);
            token_type_ids_raw.push(0); // label segment
        }
        all_tokens.push(sep_id);
        token_type_ids_raw.push(0);
    }

    let label_end_pos = all_tokens.len();

    // Encode text words and track which tokens are first subwords
    let mut words_mask_raw: Vec<i64> = vec![0; label_end_pos];

    for word in &words {
        let encoding = tokenizer
            .encode(word.text.as_str(), false)
            .map_err(|e| MlError::Tokenization(e.to_string()))?;

        let ids = encoding.get_ids();
        if ids.is_empty() {
            // Word produced no tokens; add unknown token
            all_tokens.push(100); // [UNK]
            token_type_ids_raw.push(1);
            words_mask_raw.push(1); // first (and only) subword
            continue;
        }

        for (i, &id) in ids.iter().enumerate() {
            if all_tokens.len() >= max_seq_length - 1 {
                break; // leave room for final [SEP]
            }
            all_tokens.push(id);
            token_type_ids_raw.push(1); // text segment
            words_mask_raw.push(if i == 0 { 1 } else { 0 });
        }

        if all_tokens.len() >= max_seq_length - 1 {
            break;
        }
    }

    // Final [SEP]
    all_tokens.push(sep_id);
    token_type_ids_raw.push(1);
    words_mask_raw.push(0);

    // Count actual text words that fit in the sequence
    let actual_num_words = words_mask_raw[label_end_pos..]
        .iter()
        .filter(|&&x| x == 1)
        .count();

    // Pad to max_seq_length
    let seq_length = all_tokens.len().min(max_seq_length);
    let mut input_ids = vec![0i64; max_seq_length];
    let mut attention_mask = vec![0i64; max_seq_length];
    let mut token_type_ids = vec![0i64; max_seq_length];
    let mut words_mask = vec![0i64; max_seq_length];

    for i in 0..seq_length {
        input_ids[i] = all_tokens[i] as i64;
        attention_mask[i] = 1;
        token_type_ids[i] = token_type_ids_raw[i];
        words_mask[i] = words_mask_raw[i];
    }

    // Enumerate spans over actual words
    let word_spans = enumerate_spans(actual_num_words, max_span_width);
    let span_idx: Vec<[i64; 2]> = word_spans
        .iter()
        .map(|s| [s[0] as i64, (s[1] - 1) as i64]) // GLiNER uses inclusive end
        .collect();
    let span_mask: Vec<i64> = vec![1; span_idx.len()];

    Ok(GlinerInput {
        input_ids,
        attention_mask,
        token_type_ids,
        words_mask,
        text_lengths: vec![actual_num_words as i64],
        span_idx,
        span_mask,
        num_labels,
        seq_length,
        words: words.into_iter().take(actual_num_words).collect(),
    })
}

fn get_special_token_id(tokenizer: &Tokenizer, token: &str) -> Option<u32> {
    tokenizer.token_to_id(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_into_words() {
        let words = split_into_words("Hello world  from   Rust");
        assert_eq!(words.len(), 4);
        assert_eq!(words[0].text, "Hello");
        assert_eq!(words[0].char_start, 0);
        assert_eq!(words[0].char_end, 5);
        assert_eq!(words[1].text, "world");
        assert_eq!(words[1].char_start, 6);
        assert_eq!(words[1].char_end, 11);
        assert_eq!(words[2].text, "from");
        assert_eq!(words[3].text, "Rust");
    }

    #[test]
    fn test_split_into_words_empty() {
        let words = split_into_words("");
        assert!(words.is_empty());

        let words = split_into_words("   ");
        assert!(words.is_empty());
    }

    #[test]
    fn test_split_into_words_unicode() {
        let words = split_into_words("café résumé naïve");
        assert_eq!(words.len(), 3);
        assert_eq!(words[0].text, "café");
        assert_eq!(words[1].text, "résumé");
        assert_eq!(words[2].text, "naïve");
    }

    #[test]
    fn test_enumerate_spans() {
        // 3 words, max width 2
        let spans = enumerate_spans(3, 2);
        assert_eq!(
            spans,
            vec![[0, 1], [0, 2], [1, 2], [1, 3], [2, 3]]
        );
    }

    #[test]
    fn test_enumerate_spans_single_word() {
        let spans = enumerate_spans(1, 5);
        assert_eq!(spans, vec![[0, 1]]);
    }

    #[test]
    fn test_enumerate_spans_count() {
        // For N words and max_width W:
        // Total spans = sum over i=0..N of min(W, N-i)
        let n = 10;
        let w = 4;
        let spans = enumerate_spans(n, w);
        let expected: usize = (0..n).map(|i| w.min(n - i)).sum();
        assert_eq!(spans.len(), expected);
    }

    #[test]
    fn test_enumerate_spans_empty() {
        let spans = enumerate_spans(0, 5);
        assert!(spans.is_empty());
    }

    #[test]
    fn test_preprocess_basic() {
        // Use a simple BERT tokenizer for testing
        let tokenizer = create_test_tokenizer();
        let labels = &["person", "date"];
        let text = "John went to Paris on January 5";

        let input = preprocess(text, labels, &tokenizer, 128, 6).unwrap();

        // Should have non-empty results
        assert_eq!(input.input_ids.len(), 128);
        assert_eq!(input.attention_mask.len(), 128);
        assert!(input.num_labels == 2);
        assert!(!input.span_idx.is_empty());
        assert_eq!(input.span_idx.len(), input.span_mask.len());

        // First token should be [CLS]
        assert_eq!(input.input_ids[0], 101); // [CLS]

        // Words mask should have entries for text words
        let word_count: i64 = input.words_mask.iter().sum();
        assert!(word_count > 0);
        assert_eq!(word_count, input.text_lengths[0]);
    }

    /// Create a minimal BERT tokenizer for testing via JSON
    fn create_test_tokenizer() -> Tokenizer {
        let json = r###"{
            "version": "1.0",
            "model": {
                "type": "WordPiece",
                "unk_token": "[UNK]",
                "continuing_subword_prefix": "##",
                "max_input_chars_per_word": 100,
                "vocab": {
                    "[PAD]": 0, "[UNK]": 100, "[CLS]": 101, "[SEP]": 102, "[MASK]": 103,
                    "the": 200, "a": 201, "an": 202, "is": 203, "was": 204,
                    "to": 205, "on": 206, "in": 207, "at": 208, "of": 209,
                    "john": 210, "went": 211, "paris": 212, "january": 213,
                    "person": 214, "date": 215, "invoice": 216, "number": 217,
                    "vendor": 218, "name": 219, "total": 220, "amount": 221,
                    "acme": 222, "corp": 223, "5": 224, "10": 225, "100": 226, "2024": 227
                }
            },
            "pre_tokenizer": { "type": "BertPreTokenizer" },
            "normalizer": { "type": "BertNormalizer", "clean_text": true, "handle_chinese_chars": true, "strip_accents": null, "lowercase": true }
        }"###;

        Tokenizer::from_bytes(json).expect("Failed to create test tokenizer")
    }
}
