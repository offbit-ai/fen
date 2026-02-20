use std::path::Path;
use std::time::Instant;

use super::confidence::ExtractionConfidence;
use super::config::GlinerConfig;
use super::model::GlinerModel;
use super::types::{GlinerExtractionResult, ModelTier};
use crate::error::MlError;

/// Tiered GLiNER extraction: medium model first, escalate to large if unsatisfactory
pub struct GlinerExtractor {
    config: GlinerConfig,
    medium: GlinerModel,
    large: GlinerModel,
}

impl GlinerExtractor {
    /// Create extractor without models (returns empty results)
    pub fn new(config: GlinerConfig) -> Result<Self, MlError> {
        let medium = GlinerModel::new(config.medium.clone(), ModelTier::Medium)?;
        let large = GlinerModel::new(config.large.clone(), ModelTier::Large)?;
        Ok(Self {
            config,
            medium,
            large,
        })
    }

    /// Create extractor with model files
    pub fn with_models(
        config: GlinerConfig,
        models_dir: impl AsRef<Path>,
    ) -> Result<Self, MlError> {
        let models_dir = models_dir.as_ref();

        let medium = if let (Some(model), Some(tok)) =
            (&config.medium.model_path, &config.medium.tokenizer_path)
        {
            GlinerModel::with_model(
                config.medium.clone(),
                models_dir.join(model),
                models_dir.join(tok),
                ModelTier::Medium,
            )?
        } else {
            GlinerModel::new(config.medium.clone(), ModelTier::Medium)?
        };

        let large = if let (Some(model), Some(tok)) =
            (&config.large.model_path, &config.large.tokenizer_path)
        {
            GlinerModel::with_model(
                config.large.clone(),
                models_dir.join(model),
                models_dir.join(tok),
                ModelTier::Large,
            )?
        } else {
            GlinerModel::new(config.large.clone(), ModelTier::Large)?
        };

        Ok(Self {
            config,
            medium,
            large,
        })
    }

    /// Check if at least one model is available
    pub fn has_model(&self) -> bool {
        self.medium.has_model() || self.large.has_model()
    }

    /// Check if the medium model is available
    pub fn has_medium(&self) -> bool {
        self.medium.has_model()
    }

    /// Check if the large model is available
    pub fn has_large(&self) -> bool {
        self.large.has_model()
    }

    /// Extract entities using tiered approach
    ///
    /// 1. Run medium model
    /// 2. Evaluate confidence
    /// 3. If unsatisfactory and large model available, escalate
    pub fn extract(
        &self,
        text: &str,
        labels: &[&str],
        expected_labels: &[&str],
    ) -> Result<(GlinerExtractionResult, ExtractionConfidence), MlError> {
        let start = Instant::now();
        let nms = self.config.nms_threshold;

        // Try medium model first
        if self.medium.has_model() {
            let (entities, medium_ms) = self.medium.extract(text, labels, nms)?;
            let confidence = ExtractionConfidence::compute(&entities, expected_labels);

            tracing::debug!(
                tier = "medium",
                entities = entities.len(),
                overall_confidence = %confidence.overall,
                completeness = %confidence.completeness,
                elapsed_ms = medium_ms,
                "Medium model extraction"
            );

            // Check if medium result is satisfactory
            if confidence.satisfactory(
                self.config.escalation_threshold,
                self.config.completeness_threshold,
            ) {
                return Ok((
                    GlinerExtractionResult {
                        entities,
                        tier: ModelTier::Medium,
                        escalated: false,
                        processing_time_ms: start.elapsed().as_millis() as u64,
                    },
                    confidence,
                ));
            }

            // Escalate to large model if available
            if self.large.has_model() {
                tracing::info!(
                    overall_confidence = %confidence.overall,
                    completeness = %confidence.completeness,
                    "Escalating to large model"
                );

                let (entities, _) = self.large.extract(text, labels, nms)?;
                let confidence = ExtractionConfidence::compute(&entities, expected_labels);

                return Ok((
                    GlinerExtractionResult {
                        entities,
                        tier: ModelTier::Large,
                        escalated: true,
                        processing_time_ms: start.elapsed().as_millis() as u64,
                    },
                    confidence,
                ));
            }

            // No large model — return medium result as best effort
            return Ok((
                GlinerExtractionResult {
                    entities,
                    tier: ModelTier::Medium,
                    escalated: false,
                    processing_time_ms: start.elapsed().as_millis() as u64,
                },
                confidence,
            ));
        }

        // No medium model — try large directly
        if self.large.has_model() {
            let (entities, _) = self.large.extract(text, labels, nms)?;
            let confidence = ExtractionConfidence::compute(&entities, expected_labels);

            return Ok((
                GlinerExtractionResult {
                    entities,
                    tier: ModelTier::Large,
                    escalated: false,
                    processing_time_ms: start.elapsed().as_millis() as u64,
                },
                confidence,
            ));
        }

        // No models loaded — return empty
        Ok((
            GlinerExtractionResult {
                entities: Vec::new(),
                tier: ModelTier::Medium,
                escalated: false,
                processing_time_ms: 0,
            },
            ExtractionConfidence::compute(&[], expected_labels),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extractor_without_models() {
        let config = GlinerConfig::default();
        let extractor = GlinerExtractor::new(config).unwrap();

        assert!(!extractor.has_model());
        assert!(!extractor.has_medium());
        assert!(!extractor.has_large());
    }

    #[test]
    fn test_extractor_extract_without_models() {
        let config = GlinerConfig::default();
        let extractor = GlinerExtractor::new(config).unwrap();

        let (result, confidence) = extractor
            .extract(
                "Invoice INV-001",
                &["invoice_number"],
                &["invoice_number"],
            )
            .unwrap();

        assert!(result.entities.is_empty());
        assert_eq!(confidence.overall, 0.0);
        assert!(!confidence.satisfactory_default());
    }
}
