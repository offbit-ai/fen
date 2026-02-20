use super::preprocessor::{GlinerInput, WordInfo};
use super::types::{CandidateSpan, GlinerEntity};

/// Apply sigmoid to convert logits to probabilities
#[inline]
pub fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Decode model logits into candidate spans
///
/// logits shape: [num_spans, num_labels]
/// Returns candidate spans above the threshold.
pub fn decode_logits(
    logits: &[f32],
    num_spans: usize,
    num_labels: usize,
    threshold: f32,
    input: &GlinerInput,
) -> Vec<CandidateSpan> {
    let mut candidates = Vec::new();

    for span_idx in 0..num_spans {
        if span_idx >= input.span_idx.len() {
            break;
        }
        if input.span_mask.get(span_idx).copied().unwrap_or(0) == 0 {
            continue;
        }

        let start_word = input.span_idx[span_idx][0] as usize;
        let end_word = (input.span_idx[span_idx][1] + 1) as usize; // convert back to exclusive

        for label_idx in 0..num_labels {
            let logit_idx = span_idx * num_labels + label_idx;
            if logit_idx >= logits.len() {
                break;
            }

            let score = sigmoid(logits[logit_idx]);
            if score >= threshold {
                candidates.push(CandidateSpan {
                    label_idx,
                    start_word,
                    end_word,
                    score,
                });
            }
        }
    }

    // Sort by score descending
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    candidates
}

/// Compute word-level IoU between two spans
fn span_iou(a_start: usize, a_end: usize, b_start: usize, b_end: usize) -> f32 {
    let inter_start = a_start.max(b_start);
    let inter_end = a_end.min(b_end);

    if inter_start >= inter_end {
        return 0.0;
    }

    let intersection = (inter_end - inter_start) as f32;
    let union = (a_end - a_start + b_end - b_start) as f32 - intersection;

    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// Non-maximum suppression per label
///
/// Keeps the highest-scoring span for each label and removes overlapping
/// spans with IoU above the threshold.
pub fn non_maximum_suppression(candidates: &[CandidateSpan], iou_threshold: f32) -> Vec<usize> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut kept = Vec::new();
    let mut suppressed = vec![false; candidates.len()];

    for i in 0..candidates.len() {
        if suppressed[i] {
            continue;
        }

        kept.push(i);

        // Suppress overlapping spans with the same label
        for j in (i + 1)..candidates.len() {
            if suppressed[j] {
                continue;
            }
            if candidates[j].label_idx != candidates[i].label_idx {
                continue;
            }

            let iou = span_iou(
                candidates[i].start_word,
                candidates[i].end_word,
                candidates[j].start_word,
                candidates[j].end_word,
            );

            if iou >= iou_threshold {
                suppressed[j] = true;
            }
        }
    }

    kept
}

/// Map word indices back to character offsets and build GlinerEntity list
pub fn build_entities(
    candidates: &[CandidateSpan],
    kept_indices: &[usize],
    labels: &[&str],
    words: &[WordInfo],
) -> Vec<GlinerEntity> {
    let mut entities = Vec::with_capacity(kept_indices.len());

    for &idx in kept_indices {
        let span = &candidates[idx];

        if span.label_idx >= labels.len() {
            continue;
        }
        if span.start_word >= words.len() || span.end_word > words.len() {
            continue;
        }

        let char_start = words[span.start_word].char_start;
        let char_end = words[span.end_word - 1].char_end;

        // Reconstruct text from words
        let text: String = words[span.start_word..span.end_word]
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        entities.push(GlinerEntity {
            label: labels[span.label_idx].to_string(),
            text,
            score: span.score,
            start_word: span.start_word,
            end_word: span.end_word,
            char_start,
            char_end,
        });
    }

    // Sort by score descending
    entities.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    entities
}

/// Full postprocessing pipeline: logits → entities
pub fn postprocess(
    logits: &[f32],
    num_spans: usize,
    num_labels: usize,
    threshold: f32,
    nms_threshold: f32,
    labels: &[&str],
    input: &GlinerInput,
) -> Vec<GlinerEntity> {
    let candidates = decode_logits(logits, num_spans, num_labels, threshold, input);
    let kept = non_maximum_suppression(&candidates, nms_threshold);
    build_entities(&candidates, &kept, labels, &input.words)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sigmoid() {
        assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
        assert!(sigmoid(10.0) > 0.999);
        assert!(sigmoid(-10.0) < 0.001);
        assert!((sigmoid(2.0) - 0.8808).abs() < 0.001);
    }

    #[test]
    fn test_span_iou() {
        // Identical spans
        assert!((span_iou(0, 3, 0, 3) - 1.0).abs() < 1e-6);

        // No overlap
        assert!((span_iou(0, 2, 3, 5)).abs() < 1e-6);

        // Partial overlap: [0,3) and [2,5) → intersection [2,3)=1, union=6-1=5
        assert!((span_iou(0, 3, 2, 5) - 1.0 / 5.0).abs() < 1e-6);

        // One contains the other: [0,5) and [1,3) → intersection=2, union=5
        assert!((span_iou(0, 5, 1, 3) - 2.0 / 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_nms_basic() {
        let candidates = vec![
            CandidateSpan {
                label_idx: 0,
                start_word: 0,
                end_word: 3,
                score: 0.9,
            },
            CandidateSpan {
                label_idx: 0,
                start_word: 1,
                end_word: 4,
                score: 0.7,
            }, // overlaps with [0]
            CandidateSpan {
                label_idx: 1,
                start_word: 0,
                end_word: 3,
                score: 0.8,
            }, // different label, same span
            CandidateSpan {
                label_idx: 0,
                start_word: 10,
                end_word: 12,
                score: 0.6,
            }, // no overlap
        ];

        let kept = non_maximum_suppression(&candidates, 0.3);
        // Should keep: [0] (highest for label 0), suppress [1] (overlaps with [0]),
        // keep [2] (different label), keep [3] (no overlap with [0])
        assert_eq!(kept, vec![0, 2, 3]);
    }

    #[test]
    fn test_nms_no_suppression() {
        let candidates = vec![
            CandidateSpan {
                label_idx: 0,
                start_word: 0,
                end_word: 2,
                score: 0.9,
            },
            CandidateSpan {
                label_idx: 0,
                start_word: 5,
                end_word: 7,
                score: 0.8,
            },
        ];

        let kept = non_maximum_suppression(&candidates, 0.5);
        assert_eq!(kept, vec![0, 1]); // no overlap, both kept
    }

    #[test]
    fn test_build_entities() {
        let words = vec![
            WordInfo {
                text: "Invoice".into(),
                char_start: 0,
                char_end: 7,
            },
            WordInfo {
                text: "INV-001".into(),
                char_start: 8,
                char_end: 15,
            },
            WordInfo {
                text: "from".into(),
                char_start: 16,
                char_end: 20,
            },
            WordInfo {
                text: "Acme".into(),
                char_start: 21,
                char_end: 25,
            },
            WordInfo {
                text: "Corp".into(),
                char_start: 26,
                char_end: 30,
            },
        ];

        let candidates = vec![
            CandidateSpan {
                label_idx: 0,
                start_word: 1,
                end_word: 2,
                score: 0.95,
            },
            CandidateSpan {
                label_idx: 1,
                start_word: 3,
                end_word: 5,
                score: 0.88,
            },
        ];

        let labels = &["invoice_number", "vendor_name"];
        let kept = vec![0, 1];

        let entities = build_entities(&candidates, &kept, labels, &words);
        assert_eq!(entities.len(), 2);

        assert_eq!(entities[0].label, "invoice_number");
        assert_eq!(entities[0].text, "INV-001");
        assert_eq!(entities[0].char_start, 8);
        assert_eq!(entities[0].char_end, 15);
        assert_eq!(entities[0].score, 0.95);

        assert_eq!(entities[1].label, "vendor_name");
        assert_eq!(entities[1].text, "Acme Corp");
        assert_eq!(entities[1].char_start, 21);
        assert_eq!(entities[1].char_end, 30);
    }

    #[test]
    fn test_decode_logits() {
        use super::super::preprocessor::GlinerInput;

        let input = GlinerInput {
            input_ids: vec![],
            attention_mask: vec![],
            token_type_ids: vec![],
            words_mask: vec![],
            text_lengths: vec![3],
            span_idx: vec![[0, 0], [0, 1], [1, 1], [1, 2], [2, 2]],
            span_mask: vec![1, 1, 1, 1, 1],
            num_labels: 2,
            seq_length: 10,
            words: vec![
                WordInfo {
                    text: "a".into(),
                    char_start: 0,
                    char_end: 1,
                },
                WordInfo {
                    text: "b".into(),
                    char_start: 2,
                    char_end: 3,
                },
                WordInfo {
                    text: "c".into(),
                    char_start: 4,
                    char_end: 5,
                },
            ],
        };

        // 5 spans × 2 labels = 10 logits
        // Make span[0] label[0] high, span[3] label[1] high
        let mut logits = vec![-5.0; 10];
        logits[0] = 3.0; // span 0, label 0 → sigmoid ≈ 0.95
        logits[7] = 2.0; // span 3, label 1 → sigmoid ≈ 0.88

        let candidates = decode_logits(&logits, 5, 2, 0.5, &input);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].label_idx, 0);
        assert_eq!(candidates[0].start_word, 0);
        assert_eq!(candidates[0].end_word, 1); // [0, 0] inclusive → exclusive end = 1
        assert!(candidates[0].score > 0.9);
    }

    #[test]
    fn test_full_postprocess() {
        use super::super::preprocessor::GlinerInput;

        let input = GlinerInput {
            input_ids: vec![],
            attention_mask: vec![],
            token_type_ids: vec![],
            words_mask: vec![],
            text_lengths: vec![4],
            span_idx: vec![
                [0, 0],
                [0, 1],
                [1, 1],
                [1, 2],
                [2, 2],
                [2, 3],
                [3, 3],
            ],
            span_mask: vec![1, 1, 1, 1, 1, 1, 1],
            num_labels: 2,
            seq_length: 20,
            words: vec![
                WordInfo {
                    text: "Invoice".into(),
                    char_start: 0,
                    char_end: 7,
                },
                WordInfo {
                    text: "INV-001".into(),
                    char_start: 8,
                    char_end: 15,
                },
                WordInfo {
                    text: "Acme".into(),
                    char_start: 16,
                    char_end: 20,
                },
                WordInfo {
                    text: "Corp".into(),
                    char_start: 21,
                    char_end: 25,
                },
            ],
        };

        // 7 spans × 2 labels = 14 logits
        let mut logits = vec![-5.0; 14];
        logits[2] = 3.0; // span[1]=[0,1] label[0] → "Invoice INV-001" as label 0
        logits[11] = 2.5; // span[5]=[2,3] label[1] → "Acme Corp" as label 1

        let labels = &["invoice_number", "vendor_name"];
        let entities = postprocess(&logits, 7, 2, 0.5, 0.5, labels, &input);

        assert_eq!(entities.len(), 2);
        // Entities should be sorted by score desc
        assert_eq!(entities[0].label, "invoice_number");
        assert_eq!(entities[0].text, "Invoice INV-001");
        assert_eq!(entities[1].label, "vendor_name");
        assert_eq!(entities[1].text, "Acme Corp");
    }
}
