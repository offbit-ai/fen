use rust_decimal::Decimal;

use fen_core::domain::{Anomaly, AnomalyType, Invoice, Severity, ValidationResult};

/// Built-in structural validation rules
/// These run without the rule engine for core financial validations
pub struct StructuralValidator {
    /// Tolerance for amount comparisons (to handle rounding)
    pub amount_tolerance: Decimal,
}

impl Default for StructuralValidator {
    fn default() -> Self {
        Self {
            amount_tolerance: Decimal::new(1, 2), // 0.01
        }
    }
}

impl StructuralValidator {
    /// Create a new structural validator
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate an invoice for structural consistency
    pub fn validate_invoice(&self, invoice: &Invoice) -> Vec<Anomaly> {
        let mut anomalies = Vec::new();

        // Check: Line items sum equals subtotal
        if !invoice.line_items.is_empty() {
            let line_items_sum = invoice.line_items_sum();
            let diff = (line_items_sum - invoice.subtotal).abs();

            if diff > self.amount_tolerance {
                anomalies.push(
                    Anomaly::new(
                        invoice.document_id,
                        AnomalyType::MathMismatch,
                        Severity::High,
                        "Line items do not sum to subtotal",
                    )
                    .with_field("subtotal")
                    .with_values(line_items_sum.to_string(), invoice.subtotal.to_string()),
                );
            }
        }

        // Check: Total = subtotal + tax - discount
        let expected_total = invoice.calculated_total();
        let total_diff = (expected_total - invoice.total_amount).abs();

        if total_diff > self.amount_tolerance {
            anomalies.push(
                Anomaly::new(
                    invoice.document_id,
                    AnomalyType::MathMismatch,
                    Severity::High,
                    "Total amount calculation mismatch",
                )
                .with_field("total_amount")
                .with_values(expected_total.to_string(), invoice.total_amount.to_string()),
            );
        }

        // Check: Due date after invoice date
        if let Some(due_date) = invoice.due_date {
            if due_date < invoice.invoice_date {
                anomalies.push(
                    Anomaly::new(
                        invoice.document_id,
                        AnomalyType::DateInconsistency,
                        Severity::Medium,
                        "Due date is before invoice date",
                    )
                    .with_field("due_date")
                    .with_values(
                        format!(">= {}", invoice.invoice_date),
                        due_date.to_string(),
                    ),
                );
            }
        }

        // Check: Total amount is positive
        if invoice.total_amount <= Decimal::ZERO {
            anomalies.push(
                Anomaly::new(
                    invoice.document_id,
                    AnomalyType::OutOfRange,
                    Severity::High,
                    "Invoice total must be positive",
                )
                .with_field("total_amount")
                .with_values("> 0", invoice.total_amount.to_string()),
            );
        }

        // Check: Invoice number is not empty
        if invoice.invoice_number.is_empty() || invoice.invoice_number.starts_with("UNKNOWN") {
            anomalies.push(Anomaly::new(
                invoice.document_id,
                AnomalyType::MissingField,
                Severity::Medium,
                "Invoice number could not be extracted",
            ).with_field("invoice_number"));
        }

        // Check: Vendor is not unknown
        if invoice.vendor.name == "Unknown" || invoice.vendor.name == "Unknown Vendor" {
            anomalies.push(Anomaly::new(
                invoice.document_id,
                AnomalyType::MissingField,
                Severity::Low,
                "Vendor information could not be extracted",
            ).with_field("vendor"));
        }

        // Check: Low confidence score warning
        if invoice.confidence_score < 0.7 {
            anomalies.push(
                Anomaly::new(
                    invoice.document_id,
                    AnomalyType::ValidationFailure,
                    Severity::Low,
                    format!(
                        "Low extraction confidence ({:.0}%)",
                        invoice.confidence_score * 100.0
                    ),
                )
                .with_field("confidence_score"),
            );
        }

        // Check: Line items have valid totals
        for (i, item) in invoice.line_items.iter().enumerate() {
            let expected_item_total = item.quantity * item.unit_price;
            let item_diff = (expected_item_total - item.total).abs();

            if item_diff > self.amount_tolerance {
                anomalies.push(
                    Anomaly::new(
                        invoice.document_id,
                        AnomalyType::MathMismatch,
                        Severity::Medium,
                        format!("Line item {} total mismatch", i + 1),
                    )
                    .with_field(format!("line_items[{}].total", i))
                    .with_values(expected_item_total.to_string(), item.total.to_string()),
                );
            }
        }

        anomalies
    }

    /// Build a validation result from anomalies
    pub fn build_result(&self, invoice: &Invoice, anomalies: Vec<Anomaly>, time_ms: u64) -> ValidationResult {
        let mut result = ValidationResult::new(invoice.document_id);
        result.validation_time_ms = time_ms;

        for anomaly in anomalies {
            result.add_anomaly(anomaly);
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use fen_core::domain::LineItem;

    #[test]
    fn test_valid_invoice() {
        let validator = StructuralValidator::new();
        let mut invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        invoice.line_items = vec![LineItem::new(1, "Service", Decimal::new(1, 0), Decimal::new(100, 0))];
        invoice.subtotal = Decimal::new(100, 0);
        invoice.total_amount = Decimal::new(100, 0);
        invoice.confidence_score = 0.9;

        let anomalies = validator.validate_invoice(&invoice);

        // Should only have vendor warning
        assert!(anomalies.iter().all(|a| a.severity <= Severity::Low));
    }

    #[test]
    fn test_math_mismatch() {
        let validator = StructuralValidator::new();
        let mut invoice = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        invoice.line_items = vec![LineItem::new(1, "Service", Decimal::new(1, 0), Decimal::new(100, 0))];
        invoice.subtotal = Decimal::new(200, 0); // Wrong!
        invoice.total_amount = Decimal::new(200, 0);

        let anomalies = validator.validate_invoice(&invoice);

        assert!(anomalies.iter().any(|a| a.anomaly_type == AnomalyType::MathMismatch));
    }
}
