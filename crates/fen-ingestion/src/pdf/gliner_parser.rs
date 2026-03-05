use chrono::NaiveDate;
use rust_decimal::Decimal;

use fen_core::domain::{Currency, DocumentId, Invoice, InvoiceId, Party, TenantId};
use fen_core::ValidationStatus;
use fen_ml::gliner::types::GlinerExtractionResult;

use crate::error::IngestionError;

/// Entity labels for invoice extraction
pub const INVOICE_LABELS: &[&str] = &[
    "invoice_number",
    "invoice_date",
    "due_date",
    "vendor_name",
    "bill_to_name",
    "po_number",
    "subtotal",
    "tax_amount",
    "total_amount",
    "currency",
];

/// Labels that should ideally be found for a satisfactory extraction
pub const INVOICE_EXPECTED: &[&str] = &[
    "invoice_number",
    "invoice_date",
    "vendor_name",
    "total_amount",
];

/// GLiNER-backed invoice parser: maps NER entities to Invoice domain struct
pub struct GlinerInvoiceParser;

impl GlinerInvoiceParser {
    pub fn new() -> Self {
        Self
    }

    /// Build an Invoice from GLiNER extraction results
    pub fn build(
        &self,
        result: &GlinerExtractionResult,
        document_id: DocumentId,
        text: &str,
        confidence: f32,
    ) -> Result<Invoice, IngestionError> {
        let invoice_number = result
            .best_entity("invoice_number")
            .map(|e| e.text.clone())
            .unwrap_or_else(|| {
                format!(
                    "UNKNOWN-{}",
                    uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
                )
            });

        let invoice_date = result
            .best_entity("invoice_date")
            .and_then(|e| parse_date(&e.text))
            .unwrap_or_else(|| chrono::Local::now().date_naive());

        let due_date = result
            .best_entity("due_date")
            .and_then(|e| parse_date(&e.text));

        let vendor = result
            .best_entity("vendor_name")
            .map(|e| Party::new(e.text.trim()))
            .unwrap_or_else(Party::unknown);

        let bill_to = result
            .best_entity("bill_to_name")
            .map(|e| Party::new(e.text.trim()))
            .unwrap_or_else(Party::unknown);

        let po_number = result.best_entity("po_number").map(|e| e.text.clone());

        let total_amount = result
            .best_entity("total_amount")
            .and_then(|e| parse_amount(&e.text))
            .unwrap_or(Decimal::ZERO);

        let subtotal = result
            .best_entity("subtotal")
            .and_then(|e| parse_amount(&e.text))
            .unwrap_or(Decimal::ZERO);

        let tax_amount = result
            .best_entity("tax_amount")
            .and_then(|e| parse_amount(&e.text))
            .unwrap_or(Decimal::ZERO);

        let currency = result
            .best_entity("currency")
            .map(|e| detect_currency(&e.text))
            .unwrap_or_else(|| detect_currency(text));

        Ok(Invoice {
            id: InvoiceId::new(),
            document_id,
            tenant_id: TenantId::system(),
            invoice_number,
            invoice_date,
            due_date,
            po_number,
            contract_id: None,
            contract_number: None,
            vendor,
            bill_to,
            currency,
            line_items: Vec::new(), // line items from table extraction separately
            subtotal,
            tax_amount,
            discount_amount: Decimal::ZERO,
            total_amount,
            validation_status: ValidationStatus::Pending,
            confidence_score: confidence,
            extracted_text: text.to_string(),
        })
    }
}

impl Default for GlinerInvoiceParser {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    let formats = [
        "%m/%d/%Y", "%m/%d/%y", "%d/%m/%Y", "%d/%m/%y", "%m-%d-%Y", "%m-%d-%y", "%Y-%m-%d",
        "%Y/%m/%d",
    ];

    let cleaned = s.trim().replace(['$', '€', '£'], "");

    for fmt in formats {
        if let Ok(date) = NaiveDate::parse_from_str(&cleaned, fmt) {
            return Some(date);
        }
    }

    let month_formats = ["%B %d, %Y", "%b %d, %Y", "%B %d %Y", "%b %d %Y"];
    for fmt in month_formats {
        if let Ok(date) = NaiveDate::parse_from_str(&cleaned, fmt) {
            return Some(date);
        }
    }

    None
}

fn parse_amount(s: &str) -> Option<Decimal> {
    let cleaned = s
        .replace(['$', '€', '£', '¥', ',', ' '], "")
        .trim()
        .to_string();

    let cleaned = if cleaned.starts_with('(') && cleaned.ends_with(')') {
        format!("-{}", &cleaned[1..cleaned.len() - 1])
    } else {
        cleaned
    };

    cleaned.parse().ok()
}

fn detect_currency(text: &str) -> Currency {
    let lower = text.to_lowercase();
    if text.contains('$') || lower.contains("usd") {
        Currency::USD
    } else if text.contains('€') || lower.contains("eur") {
        Currency::EUR
    } else if text.contains('£') || lower.contains("gbp") {
        Currency::GBP
    } else if lower.contains("jpy") || lower.contains("yen") {
        Currency::JPY
    } else {
        Currency::USD
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fen_ml::gliner::types::{GlinerEntity, ModelTier};

    fn make_entity(label: &str, text: &str, score: f32) -> GlinerEntity {
        GlinerEntity {
            label: label.to_string(),
            text: text.to_string(),
            score,
            start_word: 0,
            end_word: 1,
            char_start: 0,
            char_end: text.len(),
        }
    }

    #[test]
    fn test_gliner_invoice_parser_full() {
        let result = GlinerExtractionResult {
            entities: vec![
                make_entity("invoice_number", "INV-2024-001", 0.95),
                make_entity("invoice_date", "01/15/2024", 0.90),
                make_entity("due_date", "02/15/2024", 0.85),
                make_entity("vendor_name", "Acme Corporation", 0.92),
                make_entity("bill_to_name", "Widget Inc", 0.88),
                make_entity("total_amount", "$1,234.56", 0.93),
                make_entity("subtotal", "$1,100.00", 0.87),
                make_entity("tax_amount", "$134.56", 0.86),
                make_entity("po_number", "PO-5678", 0.80),
            ],
            tier: ModelTier::Medium,
            escalated: false,
            processing_time_ms: 60,
        };

        let parser = GlinerInvoiceParser::new();
        let invoice = parser
            .build(&result, DocumentId::new(), "sample text", 0.9)
            .unwrap();

        assert_eq!(invoice.invoice_number, "INV-2024-001");
        assert_eq!(
            invoice.invoice_date,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()
        );
        assert_eq!(
            invoice.due_date,
            Some(NaiveDate::from_ymd_opt(2024, 2, 15).unwrap())
        );
        assert_eq!(invoice.vendor.name, "Acme Corporation");
        assert_eq!(invoice.bill_to.name, "Widget Inc");
        assert_eq!(invoice.total_amount, Decimal::new(123456, 2));
        assert_eq!(invoice.subtotal, Decimal::new(110000, 2));
        assert_eq!(invoice.tax_amount, Decimal::new(13456, 2));
        assert_eq!(invoice.po_number, Some("PO-5678".to_string()));
        assert_eq!(invoice.confidence_score, 0.9);
    }

    #[test]
    fn test_gliner_invoice_parser_minimal() {
        let result = GlinerExtractionResult {
            entities: vec![make_entity("invoice_number", "INV-001", 0.7)],
            tier: ModelTier::Medium,
            escalated: false,
            processing_time_ms: 50,
        };

        let parser = GlinerInvoiceParser::new();
        let invoice = parser
            .build(&result, DocumentId::new(), "text", 0.4)
            .unwrap();

        assert_eq!(invoice.invoice_number, "INV-001");
        assert_eq!(invoice.total_amount, Decimal::ZERO);
        assert_eq!(invoice.vendor.name, "Unknown");
    }
}
