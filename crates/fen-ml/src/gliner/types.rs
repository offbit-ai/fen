use serde::{Deserialize, Serialize};

/// Which model tier produced the result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelTier {
    /// Medium model (first pass, ~60ms)
    Medium,
    /// Large model (escalated, ~200ms)
    Large,
}

impl std::fmt::Display for ModelTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Medium => write!(f, "medium"),
            Self::Large => write!(f, "large"),
        }
    }
}

/// A single entity extracted by GLiNER
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlinerEntity {
    /// Entity label (e.g. "invoice_number", "vendor_name")
    pub label: String,

    /// Extracted text span
    pub text: String,

    /// Sigmoid confidence score (0.0 - 1.0)
    pub score: f32,

    /// Start word index in the original text
    pub start_word: usize,

    /// End word index (exclusive) in the original text
    pub end_word: usize,

    /// Character offset start in the original text
    pub char_start: usize,

    /// Character offset end (exclusive) in the original text
    pub char_end: usize,
}

/// Result of a GLiNER extraction pass
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlinerExtractionResult {
    /// Extracted entities sorted by score descending
    pub entities: Vec<GlinerEntity>,

    /// Which model tier produced this result
    pub tier: ModelTier,

    /// Whether escalation from medium to large occurred
    pub escalated: bool,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

impl GlinerExtractionResult {
    /// Get the best entity for a given label
    pub fn best_entity(&self, label: &str) -> Option<&GlinerEntity> {
        self.entities
            .iter()
            .filter(|e| e.label == label)
            .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get all entities for a given label, sorted by score descending
    pub fn entities_for_label(&self, label: &str) -> Vec<&GlinerEntity> {
        let mut ents: Vec<_> = self.entities.iter().filter(|e| e.label == label).collect();
        ents.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        ents
    }

    /// Get the set of distinct labels found
    pub fn found_labels(&self) -> Vec<&str> {
        let mut labels: Vec<&str> = self.entities.iter().map(|e| e.label.as_str()).collect();
        labels.sort_unstable();
        labels.dedup();
        labels
    }
}

/// A candidate span before NMS filtering
#[derive(Debug, Clone)]
pub struct CandidateSpan {
    /// Label index
    pub label_idx: usize,

    /// Start word index
    pub start_word: usize,

    /// End word index (exclusive)
    pub end_word: usize,

    /// Sigmoid score
    pub score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extraction_result_best_entity() {
        let result = GlinerExtractionResult {
            entities: vec![
                GlinerEntity {
                    label: "invoice_number".into(),
                    text: "INV-001".into(),
                    score: 0.9,
                    start_word: 0,
                    end_word: 1,
                    char_start: 0,
                    char_end: 7,
                },
                GlinerEntity {
                    label: "invoice_number".into(),
                    text: "INV-002".into(),
                    score: 0.7,
                    start_word: 5,
                    end_word: 6,
                    char_start: 30,
                    char_end: 37,
                },
                GlinerEntity {
                    label: "vendor_name".into(),
                    text: "Acme Corp".into(),
                    score: 0.85,
                    start_word: 2,
                    end_word: 4,
                    char_start: 10,
                    char_end: 19,
                },
            ],
            tier: ModelTier::Medium,
            escalated: false,
            processing_time_ms: 50,
        };

        let best = result.best_entity("invoice_number").unwrap();
        assert_eq!(best.text, "INV-001");
        assert_eq!(best.score, 0.9);

        let all_inv = result.entities_for_label("invoice_number");
        assert_eq!(all_inv.len(), 2);
        assert_eq!(all_inv[0].text, "INV-001"); // highest score first

        let labels = result.found_labels();
        assert_eq!(labels, vec!["invoice_number", "vendor_name"]);

        assert!(result.best_entity("nonexistent").is_none());
    }
}
