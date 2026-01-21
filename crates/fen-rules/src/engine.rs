use std::time::Instant;

use fen_core::domain::{Invoice, ValidationResult};

use crate::error::RuleError;
use crate::structural::StructuralValidator;

/// Rule engine for invoice validation
/// Currently uses built-in structural validations only
/// Note: zen-engine GoRules integration is disabled due to thread-safety constraints
/// (rquickjs uses Rc which isn't Send). Can be re-enabled with spawn_blocking in future.
pub struct RuleEngine {
    structural_validator: StructuralValidator,
}

impl RuleEngine {
    /// Create a new rule engine
    pub async fn new(_rules_path: Option<&std::path::Path>) -> Result<Self, RuleError> {
        // Note: zen-engine integration disabled for now due to !Send constraints
        // Rules path is ignored until we implement proper thread isolation
        Ok(Self {
            structural_validator: StructuralValidator::new(),
        })
    }

    /// Create a rule engine with only built-in validations
    pub fn builtin_only() -> Self {
        Self {
            structural_validator: StructuralValidator::new(),
        }
    }

    /// Validate an invoice using structural validations
    pub async fn validate_invoice(&self, invoice: &Invoice) -> Result<ValidationResult, RuleError> {
        let start = Instant::now();

        // Run structural validations
        let anomalies = self.structural_validator.validate_invoice(invoice);

        let validation_time_ms = start.elapsed().as_millis() as u64;
        let result = self
            .structural_validator
            .build_result(invoice, anomalies, validation_time_ms);

        tracing::debug!(
            invoice_id = %invoice.id,
            is_valid = result.is_valid,
            anomaly_count = result.anomalies.len(),
            time_ms = validation_time_ms,
            "Validation completed"
        );

        Ok(result)
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
}
