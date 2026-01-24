use std::path::Path;
use std::time::Instant;

use fen_core::domain::{Anomaly, AnomalyType, Invoice, Severity, ValidationResult};

use crate::error::RuleError;
use crate::structural::StructuralValidator;
use crate::zen::ZenEngineHandle;

/// Rule engine for invoice validation
/// Combines built-in structural validations with GoRules Zen for custom rules
pub struct RuleEngine {
    structural_validator: StructuralValidator,
    /// Optional Zen engine for custom JDM rules
    zen_engine: Option<ZenEngineHandle>,
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
        })
    }

    /// Create a rule engine with only built-in validations
    pub fn builtin_only() -> Self {
        Self {
            structural_validator: StructuralValidator::new(),
            zen_engine: None,
        }
    }

    /// Check if custom GoRules are loaded
    pub fn has_custom_rules(&self) -> bool {
        self.zen_engine.is_some()
    }

    /// Validate an invoice using structural validations and optional custom rules
    pub async fn validate_invoice(&self, invoice: &Invoice) -> Result<ValidationResult, RuleError> {
        let start = Instant::now();

        // Run structural validations
        let mut anomalies = self.structural_validator.validate_invoice(invoice);

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
            "Validation completed"
        );

        Ok(result)
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
    use chrono::NaiveDate;
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
}
