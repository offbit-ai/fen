use std::path::Path;
use std::time::Instant;

use fen_core::domain::{Anomaly, AnomalyType, Invoice, Severity, ValidationResult, VendorBaseline};

use crate::error::RuleError;
use crate::statistical::{StatisticalAnalyzer, StatisticalAnalyzerConfig};
use crate::structural::StructuralValidator;
use crate::zen::ZenEngineHandle;

/// Rule engine for invoice validation
/// Combines built-in structural validations with GoRules Zen for custom rules
/// and optional statistical anomaly detection
pub struct RuleEngine {
    structural_validator: StructuralValidator,
    /// Optional Zen engine for custom JDM rules
    zen_engine: Option<ZenEngineHandle>,
    /// Optional statistical analyzer for baseline-based detection
    statistical_analyzer: Option<StatisticalAnalyzer>,
}

impl RuleEngine {
    /// Create a new rule engine, optionally loading custom rules
    pub async fn new(rules_path: Option<&Path>) -> Result<Self, RuleError> {
        let zen_engine = if let Some(path) = rules_path {
            if path.exists() {
                match ZenEngineHandle::new(path).await {
                    Ok(engine) => {
                        tracing::info!(path = %path.display(), "Loaded GoRules decision");
                        Some(engine)
                    }
                    Err(e) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "Failed to load GoRules decision, using built-in rules only"
                        );
                        None
                    }
                }
            } else {
                tracing::debug!(path = %path.display(), "Rules file not found, using built-in rules only");
                None
            }
        } else {
            None
        };

        Ok(Self {
            structural_validator: StructuralValidator::new(),
            zen_engine,
            statistical_analyzer: None,
        })
    }

    /// Create a rule engine with only built-in validations
    pub fn builtin_only() -> Self {
        Self {
            structural_validator: StructuralValidator::new(),
            zen_engine: None,
            statistical_analyzer: None,
        }
    }

    /// Enable statistical analysis with default configuration
    pub fn with_statistical_analysis(mut self) -> Self {
        self.statistical_analyzer = Some(StatisticalAnalyzer::new());
        self
    }

    /// Enable statistical analysis with custom configuration
    pub fn with_statistical_config(mut self, config: StatisticalAnalyzerConfig) -> Self {
        self.statistical_analyzer = Some(StatisticalAnalyzer::with_config(config));
        self
    }

    /// Check if statistical analysis is enabled
    pub fn has_statistical_analysis(&self) -> bool {
        self.statistical_analyzer.is_some()
    }

    /// Check if custom GoRules are loaded
    pub fn has_custom_rules(&self) -> bool {
        self.zen_engine.is_some()
    }

    /// Validate an invoice using structural validations and optional custom rules
    pub async fn validate_invoice(&self, invoice: &Invoice) -> Result<ValidationResult, RuleError> {
        self.validate_invoice_with_baselines(invoice, &[]).await
    }

    /// Validate an invoice with statistical analysis using provided baselines
    pub async fn validate_invoice_with_baselines(
        &self,
        invoice: &Invoice,
        baselines: &[VendorBaseline],
    ) -> Result<ValidationResult, RuleError> {
        let start = Instant::now();

        // Run structural validations
        let mut anomalies = self.structural_validator.validate_invoice(invoice);

        // Run statistical analysis if enabled and baselines provided
        if let Some(analyzer) = &self.statistical_analyzer {
            if !baselines.is_empty() {
                let statistical_anomalies = analyzer.analyze_for_anomalies(invoice, baselines);
                if !statistical_anomalies.is_empty() {
                    tracing::debug!(
                        invoice_id = %invoice.id,
                        outlier_count = statistical_anomalies.len(),
                        "Statistical outliers detected"
                    );
                }
                anomalies.extend(statistical_anomalies);
            }
        }

        // Run custom rules if available
        if let Some(zen) = &self.zen_engine {
            let custom_anomalies = self.run_custom_rules(zen, invoice).await?;
            anomalies.extend(custom_anomalies);
        }

        let validation_time_ms = start.elapsed().as_millis() as u64;
        let result = self
            .structural_validator
            .build_result(invoice, anomalies, validation_time_ms);

        tracing::debug!(
            invoice_id = %invoice.id,
            is_valid = result.is_valid,
            anomaly_count = result.anomalies.len(),
            time_ms = validation_time_ms,
            has_custom_rules = self.has_custom_rules(),
            has_statistical = self.has_statistical_analysis(),
            "Validation completed"
        );

        Ok(result)
    }

    /// Get the statistical analyzer (if enabled)
    pub fn statistical_analyzer(&self) -> Option<&StatisticalAnalyzer> {
        self.statistical_analyzer.as_ref()
    }

    /// Run custom GoRules validations
    async fn run_custom_rules(
        &self,
        zen: &ZenEngineHandle,
        invoice: &Invoice,
    ) -> Result<Vec<Anomaly>, RuleError> {
        // Convert invoice to JSON - Invoice implements Serialize
        let input = serde_json::to_value(invoice)?;

        let result = zen.evaluate(input).await?;

        // Parse rule violations from result
        let mut anomalies = Vec::new();

        if let Some(violations) = result.get("violations").and_then(|v| v.as_array()) {
            for violation in violations {
                let message = violation
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Custom rule violation");

                let severity_str = violation
                    .get("severity")
                    .and_then(|s| s.as_str())
                    .unwrap_or("medium");

                let severity = match severity_str.to_lowercase().as_str() {
                    "critical" | "high" => Severity::High,
                    "low" | "warning" => Severity::Low,
                    _ => Severity::Medium,
                };

                let field = violation
                    .get("field")
                    .and_then(|f| f.as_str())
                    .map(|s| s.to_string());

                let mut anomaly = Anomaly::new(
                    invoice.document_id,
                    AnomalyType::ValidationFailure,
                    severity,
                    message,
                );

                if let Some(f) = field {
                    anomaly = anomaly.with_field(f);
                }

                anomalies.push(anomaly);
            }
        }

        Ok(anomalies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Utc};
    use fen_core::domain::{BaselineId, BaselinePeriod, BaselineStats};
    use rust_decimal::Decimal;

    #[tokio::test]
    async fn test_builtin_validation() {
        let engine = RuleEngine::builtin_only();

        let mut invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        invoice.subtotal = Decimal::new(100, 0);
        invoice.total_amount = Decimal::new(100, 0);
        invoice.confidence_score = 0.9;

        let result = engine.validate_invoice(&invoice).await.unwrap();

        // Should pass with minor warnings
        assert!(!result.has_critical());
    }

    #[tokio::test]
    async fn test_no_custom_rules_by_default() {
        let engine = RuleEngine::builtin_only();
        assert!(!engine.has_custom_rules());
    }

    #[tokio::test]
    async fn test_statistical_analysis_enabled() {
        let engine = RuleEngine::builtin_only().with_statistical_analysis();
        assert!(engine.has_statistical_analysis());
    }

    #[tokio::test]
    async fn test_statistical_outlier_detection() {
        let engine = RuleEngine::builtin_only().with_statistical_analysis();

        let mut invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        invoice.subtotal = Decimal::new(5000, 0); // $5000 - an outlier
        invoice.total_amount = Decimal::new(5000, 0);
        invoice.confidence_score = 0.9;

        // Create baseline with mean $1000, stddev $100
        let baseline = VendorBaseline {
            id: BaselineId::new(),
            vendor_name: invoice.vendor.name.clone(),
            metric_name: "total_amount".to_string(),
            period: BaselinePeriod::Rolling90Days,
            stats: BaselineStats {
                count: 50,
                mean: 1000.0,
                stddev: 100.0,
                min: 700.0,
                max: 1300.0,
                p25: 933.0,
                p50: 1000.0,
                p75: 1067.0,
                p90: 1128.0,
                p95: 1165.0,
                p99: 1233.0,
            },
            recent_values: vec![950.0, 1000.0, 1050.0],
            computed_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(24),
        };

        let result = engine
            .validate_invoice_with_baselines(&invoice, &[baseline])
            .await
            .unwrap();

        // Should have statistical outlier anomaly
        let statistical_anomalies: Vec<_> = result
            .anomalies
            .iter()
            .filter(|a| a.anomaly_type == AnomalyType::StatisticalOutlier)
            .collect();

        assert_eq!(statistical_anomalies.len(), 1);
        let anomaly = statistical_anomalies[0];
        assert!(anomaly.statistical_score.is_some());

        let score = anomaly.statistical_score.as_ref().unwrap();
        assert!(score.is_outlier);
        assert!(score.z_score > 2.0); // Should be ~40 stddev above mean
    }

    #[tokio::test]
    async fn test_statistical_normal_value() {
        let engine = RuleEngine::builtin_only().with_statistical_analysis();

        let mut invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        invoice.subtotal = Decimal::new(1050, 0); // $1050 - within normal range
        invoice.total_amount = Decimal::new(1050, 0);
        invoice.confidence_score = 0.9;

        let baseline = VendorBaseline {
            id: BaselineId::new(),
            vendor_name: invoice.vendor.name.clone(),
            metric_name: "total_amount".to_string(),
            period: BaselinePeriod::Rolling90Days,
            stats: BaselineStats {
                count: 50,
                mean: 1000.0,
                stddev: 100.0,
                min: 700.0,
                max: 1300.0,
                p25: 933.0,
                p50: 1000.0,
                p75: 1067.0,
                p90: 1128.0,
                p95: 1165.0,
                p99: 1233.0,
            },
            recent_values: vec![950.0, 1000.0, 1050.0],
            computed_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::hours(24),
        };

        let result = engine
            .validate_invoice_with_baselines(&invoice, &[baseline])
            .await
            .unwrap();

        // Should NOT have statistical outlier anomaly
        let statistical_anomalies: Vec<_> = result
            .anomalies
            .iter()
            .filter(|a| a.anomaly_type == AnomalyType::StatisticalOutlier)
            .collect();

        assert!(statistical_anomalies.is_empty());
    }
}
