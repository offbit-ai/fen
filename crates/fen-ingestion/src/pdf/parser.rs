use chrono::NaiveDate;
use regex::Regex;
use rust_decimal::Decimal;

use fen_core::domain::{Currency, DocumentId, Invoice, InvoiceId, LineItem, Party};
use fen_core::ValidationStatus;

use crate::error::IngestionError;

/// Basic invoice parser using regex patterns
/// In Phase 2, this will be replaced by LayoutLMv3
pub struct InvoiceParser {
    invoice_number_patterns: Vec<Regex>,
    date_patterns: Vec<Regex>,
    amount_pattern: Regex,
    vendor_pattern: Regex,
}

impl InvoiceParser {
    pub fn new() -> Self {
        Self {
            invoice_number_patterns: vec![
                Regex::new(r"(?i)invoice\s*#?\s*:?\s*([A-Z0-9][-A-Z0-9]+)").unwrap(),
                Regex::new(r"(?i)inv\s*#?\s*:?\s*([A-Z0-9][-A-Z0-9]+)").unwrap(),
                Regex::new(r"(?i)invoice\s+number\s*:?\s*([A-Z0-9][-A-Z0-9]+)").unwrap(),
                Regex::new(r"(?i)bill\s*#?\s*:?\s*([A-Z0-9][-A-Z0-9]+)").unwrap(),
            ],
            date_patterns: vec![
                Regex::new(r"(\d{1,2}[/-]\d{1,2}[/-]\d{2,4})").unwrap(),
                Regex::new(r"(\d{4}[/-]\d{1,2}[/-]\d{1,2})").unwrap(),
                Regex::new(r"(?i)(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]*\s+\d{1,2},?\s+\d{4}").unwrap(),
            ],
            amount_pattern: Regex::new(r"\$\s*([\d,]+\.?\d*)").unwrap(),
            vendor_pattern: Regex::new(r"(?i)(?:from|vendor|supplier|billed\s+by)\s*:?\s*(.+?)(?:\n|$)").unwrap(),
        }
    }

    /// Parse invoice from extracted text
    pub fn parse(&self, text: &str, document_id: DocumentId) -> Result<Invoice, IngestionError> {
        // Extract invoice number
        let invoice_number = self.extract_invoice_number(text);

        // Extract date (simplified - takes first date found)
        let invoice_date = self.extract_date(text).unwrap_or_else(|| chrono::Local::now().date_naive());

        // Extract total amount (takes largest amount as total)
        let total_amount = self.extract_total_amount(text);

        // Extract vendor name
        let vendor_name = self.extract_vendor(text);

        // Create vendor party
        let vendor = Party::new(vendor_name);

        Ok(Invoice {
            id: InvoiceId::new(),
            document_id,
            invoice_number,
            invoice_date,
            due_date: None,
            po_number: self.extract_po_number(text),
            vendor,
            bill_to: Party::unknown(),
            currency: Currency::USD,
            line_items: self.extract_line_items(text),
            subtotal: total_amount,
            tax_amount: Decimal::ZERO,
            discount_amount: Decimal::ZERO,
            total_amount,
            validation_status: ValidationStatus::Pending,
            confidence_score: 0.5, // Low confidence for regex-based extraction
            extracted_text: text.to_string(),
        })
    }

    fn extract_invoice_number(&self, text: &str) -> String {
        for pattern in &self.invoice_number_patterns {
            if let Some(caps) = pattern.captures(text) {
                if let Some(m) = caps.get(1) {
                    return m.as_str().to_string();
                }
            }
        }
        // Generate a fallback invoice number
        format!("UNKNOWN-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap())
    }

    fn extract_date(&self, text: &str) -> Option<NaiveDate> {
        for pattern in &self.date_patterns {
            if let Some(caps) = pattern.captures(text) {
                if let Some(m) = caps.get(0) {
                    if let Some(date) = parse_date(m.as_str()) {
                        return Some(date);
                    }
                }
            }
        }
        None
    }

    fn extract_total_amount(&self, text: &str) -> Decimal {
        let amounts: Vec<Decimal> = self
            .amount_pattern
            .captures_iter(text)
            .filter_map(|c| c.get(1))
            .filter_map(|m| parse_amount(m.as_str()))
            .collect();

        // Return the largest amount (likely the total)
        amounts.into_iter().max().unwrap_or(Decimal::ZERO)
    }

    fn extract_vendor(&self, text: &str) -> String {
        if let Some(caps) = self.vendor_pattern.captures(text) {
            if let Some(m) = caps.get(1) {
                let vendor = m.as_str().trim();
                if !vendor.is_empty() {
                    return vendor.to_string();
                }
            }
        }
        "Unknown Vendor".to_string()
    }

    fn extract_po_number(&self, text: &str) -> Option<String> {
        let po_pattern = Regex::new(r"(?i)(?:po|purchase\s+order)\s*#?\s*:?\s*([A-Z0-9][-A-Z0-9]+)").unwrap();
        po_pattern
            .captures(text)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
    }

    fn extract_line_items(&self, text: &str) -> Vec<LineItem> {
        // Simple line item extraction - looks for patterns like "Description $Amount"
        let item_pattern = Regex::new(r"(?m)^(.{10,50}?)\s+\$\s*([\d,]+\.?\d*)").unwrap();

        let mut items = Vec::new();
        for (i, caps) in item_pattern.captures_iter(text).enumerate() {
            if let (Some(desc), Some(amount)) = (caps.get(1), caps.get(2)) {
                if let Some(total) = parse_amount(amount.as_str()) {
                    items.push(LineItem::new(
                        (i + 1) as u32,
                        desc.as_str().trim(),
                        Decimal::ONE,
                        total,
                    ));
                }
            }
        }
        items
    }
}

impl Default for InvoiceParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse various date formats
fn parse_date(s: &str) -> Option<NaiveDate> {
    let formats = [
        "%m/%d/%Y",
        "%m/%d/%y",
        "%d/%m/%Y",
        "%d/%m/%y",
        "%m-%d-%Y",
        "%m-%d-%y",
        "%Y-%m-%d",
        "%Y/%m/%d",
    ];

    let cleaned = s.trim();

    for fmt in formats {
        if let Ok(date) = NaiveDate::parse_from_str(cleaned, fmt) {
            return Some(date);
        }
    }

    // Try parsing "Month Day, Year" format
    let month_formats = ["%B %d, %Y", "%b %d, %Y", "%B %d %Y", "%b %d %Y"];
    for fmt in month_formats {
        if let Ok(date) = NaiveDate::parse_from_str(cleaned, fmt) {
            return Some(date);
        }
    }

    None
}

/// Parse currency amount
fn parse_amount(s: &str) -> Option<Decimal> {
    let cleaned = s.replace(',', "").trim().to_string();
    cleaned.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_date_us_format() {
        assert_eq!(
            parse_date("01/15/2024"),
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        );
    }

    #[test]
    fn test_parse_date_iso_format() {
        assert_eq!(
            parse_date("2024-01-15"),
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        );
    }

    #[test]
    fn test_parse_amount() {
        assert_eq!(parse_amount("1,234.56"), Some(Decimal::new(123456, 2)));
        assert_eq!(parse_amount("100"), Some(Decimal::new(100, 0)));
    }

    #[test]
    fn test_extract_invoice_number() {
        let parser = InvoiceParser::new();
        let text = "Invoice #: INV-2024-001\nDate: 01/15/2024";
        assert_eq!(parser.extract_invoice_number(text), "INV-2024-001");
    }
}
