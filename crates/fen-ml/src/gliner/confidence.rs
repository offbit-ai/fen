use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::types::GlinerEntity;

/// Extraction confidence assessment computed from GLiNER results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionConfidence {
    /// Mean entity score across all extracted entities
    pub mean_entity_score: f32,

    /// Minimum entity score (weakest link)
    pub min_entity_score: f32,

    /// Completeness: fraction of expected entity types that were found
    pub completeness: f32,

    /// Number of entities with score below 0.5
    pub low_confidence_count: usize,

    /// Composite overall confidence score
    pub overall: f32,

    /// Per-label best confidence score
    pub per_label: HashMap<String, f32>,
}

impl ExtractionConfidence {
    /// Compute confidence from extraction results
    ///
    /// - `entities`: the extracted entities
    /// - `expected_labels`: labels that should ideally be found (e.g. invoice_number, vendor_name)
    pub fn compute(entities: &[GlinerEntity], expected_labels: &[&str]) -> Self {
        if entities.is_empty() {
            return Self {
                mean_entity_score: 0.0,
                min_entity_score: 0.0,
                completeness: 0.0,
                low_confidence_count: 0,
                overall: 0.0,
                per_label: HashMap::new(),
            };
        }

        // Per-label best scores
        let mut per_label: HashMap<String, f32> = HashMap::new();
        for entity in entities {
            let entry = per_label.entry(entity.label.clone()).or_insert(0.0);
            if entity.score > *entry {
                *entry = entity.score;
            }
        }

        // Mean and min scores
        let scores: Vec<f32> = entities.iter().map(|e| e.score).collect();
        let mean_entity_score = scores.iter().sum::<f32>() / scores.len() as f32;
        let min_entity_score = scores.iter().copied().fold(f32::INFINITY, f32::min);

        // Completeness: fraction of expected labels that were found
        let found_count = expected_labels
            .iter()
            .filter(|&&label| per_label.contains_key(label))
            .count();
        let completeness = if expected_labels.is_empty() {
            1.0
        } else {
            found_count as f32 / expected_labels.len() as f32
        };

        // Low confidence count
        let low_confidence_count = entities.iter().filter(|e| e.score < 0.5).count();

        // Composite score:
        // 40% mean entity score
        // 30% completeness
        // 20% min entity score
        // 10% penalty for low-confidence entities
        let low_conf_penalty = if entities.is_empty() {
            0.0
        } else {
            1.0 - (low_confidence_count as f32 / entities.len() as f32)
        };

        let overall = 0.4 * mean_entity_score
            + 0.3 * completeness
            + 0.2 * min_entity_score
            + 0.1 * low_conf_penalty;

        Self {
            mean_entity_score,
            min_entity_score,
            completeness,
            low_confidence_count,
            overall,
            per_label,
        }
    }

    /// Check if extraction quality is satisfactory (no escalation needed)
    pub fn satisfactory(&self, escalation_threshold: f32, completeness_threshold: f32) -> bool {
        self.overall >= escalation_threshold
            && self.completeness >= completeness_threshold
            && self.min_entity_score >= 0.3
    }

    /// Check with default thresholds (0.65 overall, 0.6 completeness)
    pub fn satisfactory_default(&self) -> bool {
        self.satisfactory(0.65, 0.6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gliner::types::GlinerEntity;

    fn make_entity(label: &str, score: f32) -> GlinerEntity {
        GlinerEntity {
            label: label.to_string(),
            text: "test".to_string(),
            score,
            start_word: 0,
            end_word: 1,
            char_start: 0,
            char_end: 4,
        }
    }

    #[test]
    fn test_confidence_high_quality() {
        let entities = vec![
            make_entity("invoice_number", 0.95),
            make_entity("vendor_name", 0.90),
            make_entity("total_amount", 0.88),
            make_entity("invoice_date", 0.85),
        ];
        let expected = &[
            "invoice_number",
            "vendor_name",
            "total_amount",
            "invoice_date",
        ];

        let conf = ExtractionConfidence::compute(&entities, expected);

        assert!(conf.mean_entity_score > 0.85);
        assert!((conf.completeness - 1.0).abs() < 1e-6);
        assert_eq!(conf.low_confidence_count, 0);
        assert!(conf.overall > 0.8);
        assert!(conf.satisfactory_default());
    }

    #[test]
    fn test_confidence_low_quality() {
        let entities = vec![
            make_entity("invoice_number", 0.4),
            make_entity("vendor_name", 0.35),
        ];
        let expected = &[
            "invoice_number",
            "vendor_name",
            "total_amount",
            "invoice_date",
        ];

        let conf = ExtractionConfidence::compute(&entities, expected);

        assert!(conf.mean_entity_score < 0.5);
        assert!((conf.completeness - 0.5).abs() < 1e-6);
        assert_eq!(conf.low_confidence_count, 2);
        assert!(!conf.satisfactory_default());
    }

    #[test]
    fn test_confidence_empty_entities() {
        let conf = ExtractionConfidence::compute(&[], &["invoice_number"]);

        assert_eq!(conf.mean_entity_score, 0.0);
        assert_eq!(conf.completeness, 0.0);
        assert_eq!(conf.overall, 0.0);
        assert!(!conf.satisfactory_default());
    }

    #[test]
    fn test_confidence_no_expected_labels() {
        let entities = vec![make_entity("something", 0.8)];
        let conf = ExtractionConfidence::compute(&entities, &[]);

        assert!((conf.completeness - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_confidence_partial_completeness() {
        let entities = vec![
            make_entity("invoice_number", 0.9),
            make_entity("vendor_name", 0.85),
        ];
        let expected = &["invoice_number", "vendor_name", "total_amount"];

        let conf = ExtractionConfidence::compute(&entities, expected);

        // 2 out of 3 expected
        assert!((conf.completeness - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_confidence_min_score_gate() {
        // High mean but one very low score → min_entity_score gate fails
        let entities = vec![
            make_entity("invoice_number", 0.95),
            make_entity("vendor_name", 0.20), // below 0.3 gate
            make_entity("total_amount", 0.90),
        ];
        let expected = &["invoice_number", "vendor_name", "total_amount"];

        let conf = ExtractionConfidence::compute(&entities, expected);

        assert!(conf.min_entity_score < 0.3);
        assert!(!conf.satisfactory_default()); // fails min_entity_score gate
    }

    #[test]
    fn test_confidence_per_label() {
        let entities = vec![
            make_entity("invoice_number", 0.7),
            make_entity("invoice_number", 0.9), // higher
            make_entity("vendor_name", 0.85),
        ];

        let conf = ExtractionConfidence::compute(&entities, &[]);

        assert_eq!(conf.per_label["invoice_number"], 0.9); // best score
        assert_eq!(conf.per_label["vendor_name"], 0.85);
    }
}
