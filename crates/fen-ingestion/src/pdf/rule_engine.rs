use chrono::NaiveDate;
use rust_decimal::Decimal;

use fen_core::domain::Currency;

/// Deterministic, domain-specific extraction layer for invoice fields.
/// Runs after GLiNER in the fallback chain, before bare regex.
/// Uses contextual line scanning — matching label + value proximity
/// rather than scanning the entire document with loose patterns.
pub struct InvoiceRuleEngine;

/// Result of rule-based extraction. Each field is `None` if no rule matched.
#[derive(Debug, Default)]
pub struct RuleExtractionResult {
    pub invoice_number: Option<String>,
    pub invoice_date: Option<NaiveDate>,
    pub due_date: Option<NaiveDate>,
    pub total_amount: Option<Decimal>,
    pub subtotal: Option<Decimal>,
    pub tax_amount: Option<Decimal>,
    pub vendor: Option<String>,
    pub bill_to: Option<String>,
    pub po_number: Option<String>,
    pub currency: Option<Currency>,
}

impl InvoiceRuleEngine {
    pub fn new() -> Self {
        Self
    }

    /// Extract all fields using domain-specific rules.
    pub fn extract(&self, text: &str) -> RuleExtractionResult {
        let lines: Vec<&str> = text.lines().collect();

        RuleExtractionResult {
            invoice_number: self.extract_invoice_number(&lines),
            invoice_date: self.extract_labeled_date(&lines, &["invoice date", "date"]),
            due_date: self.extract_labeled_date(&lines, &["due date", "payment date", "pay by"]),
            total_amount: self.extract_labeled_amount(&lines, &["total", "amount due", "balance due", "grand total"]),
            subtotal: self.extract_labeled_amount(&lines, &["subtotal", "sub total", "sub-total"]),
            tax_amount: self.extract_labeled_amount(&lines, &["tax", "vat", "gst", "sales tax"]),
            vendor: self.extract_vendor(&lines),
            bill_to: self.extract_section_value(&lines, &["bill to", "customer", "client", "buyer", "sold to"]),
            po_number: self.extract_po_number(&lines),
            currency: self.detect_currency(text),
        }
    }

    fn extract_invoice_number(&self, lines: &[&str]) -> Option<String> {
        let labels = [
            "invoice #", "invoice no", "invoice number", "inv #", "inv no",
            "invoice#", "invoice:", "inv:",
        ];

        for (i, line) in lines.iter().enumerate() {
            let lower = line.to_lowercase();
            for label in &labels {
                if let Some(pos) = lower.find(label) {
                    // Value on same line after label
                    let after = &line[pos + label.len()..];
                    let value = after.trim_start_matches([':', ' ', '#', '\t']).trim();
                    if !value.is_empty() && value.len() <= 40 {
                        return Some(value.to_string());
                    }
                    // Value on next line
                    if let Some(next) = lines.get(i + 1) {
                        let next_trimmed = next.trim();
                        if !next_trimmed.is_empty() && next_trimmed.len() <= 40 {
                            return Some(next_trimmed.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_labeled_date(&self, lines: &[&str], labels: &[&str]) -> Option<NaiveDate> {
        for (i, line) in lines.iter().enumerate() {
            let lower = line.to_lowercase();
            for label in labels {
                if !lower.contains(label) {
                    continue;
                }
                // Skip if this is a "due date" line but we're looking for "date" (not "due")
                if *label == "date" && lower.contains("due") {
                    continue;
                }

                if let Some(pos) = lower.find(label) {
                    let after = &line[pos + label.len()..];
                    let value = after.trim_start_matches([':', ' ', '\t']).trim();
                    if let Some(date) = parse_date(value) {
                        return Some(date);
                    }
                }
                // Try next line
                if let Some(next) = lines.get(i + 1) {
                    if let Some(date) = parse_date(next.trim()) {
                        return Some(date);
                    }
                }
            }
        }
        None
    }

    fn extract_labeled_amount(&self, lines: &[&str], labels: &[&str]) -> Option<Decimal> {
        for (i, line) in lines.iter().enumerate() {
            let lower = line.to_lowercase();
            for label in labels {
                if !lower.contains(label) {
                    continue;
                }
                // For "total", skip lines with "subtotal" or "sub total"
                if *label == "total"
                    && (lower.contains("subtotal") || lower.contains("sub total") || lower.contains("sub-total"))
                {
                    continue;
                }

                if let Some(pos) = lower.find(label) {
                    let after = &line[pos + label.len()..];
                    let value = after.trim_start_matches([':', ' ', '\t']).trim();
                    if let Some(amount) = parse_amount(value) {
                        return Some(amount);
                    }
                }
                // Try next line
                if let Some(next) = lines.get(i + 1) {
                    if let Some(amount) = parse_amount(next.trim()) {
                        return Some(amount);
                    }
                }
            }
        }
        None
    }

    fn extract_vendor(&self, lines: &[&str]) -> Option<String> {
        // Strategy 1: labeled vendor
        let labeled = self.extract_section_value(
            lines,
            &["from", "vendor", "supplier", "seller", "sold by", "billed by"],
        );
        if labeled.is_some() {
            return labeled;
        }

        // Strategy 2: first non-blank, non-numeric line (header heuristic)
        for line in lines.iter().take(5) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Skip lines that look like dates, amounts, or invoice labels
            let lower = trimmed.to_lowercase();
            if lower.contains("invoice")
                || lower.contains("date")
                || lower.contains("total")
                || lower.starts_with('$')
                || trimmed.parse::<f64>().is_ok()
            {
                continue;
            }
            if trimmed.len() >= 3 && trimmed.len() <= 80 {
                return Some(trimmed.to_string());
            }
        }
        None
    }

    fn extract_section_value(&self, lines: &[&str], labels: &[&str]) -> Option<String> {
        for (i, line) in lines.iter().enumerate() {
            let lower = line.to_lowercase();
            for label in labels {
                if !lower.contains(label) {
                    continue;
                }
                if let Some(pos) = lower.find(label) {
                    let after = &line[pos + label.len()..];
                    let value = after.trim_start_matches([':', ' ', '\t']).trim();
                    if !value.is_empty() && value.len() <= 80 {
                        return Some(value.to_string());
                    }
                    // Value on next line
                    if let Some(next) = lines.get(i + 1) {
                        let next_trimmed = next.trim();
                        if !next_trimmed.is_empty() && next_trimmed.len() <= 80 {
                            return Some(next_trimmed.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_po_number(&self, lines: &[&str]) -> Option<String> {
        let labels = ["po #", "po no", "po number", "purchase order", "p.o."];
        for (i, line) in lines.iter().enumerate() {
            let lower = line.to_lowercase();
            for label in &labels {
                if let Some(pos) = lower.find(label) {
                    let after = &line[pos + label.len()..];
                    let value = after.trim_start_matches([':', ' ', '#', '\t']).trim();
                    if !value.is_empty() && value.len() <= 40 {
                        return Some(value.to_string());
                    }
                    if let Some(next) = lines.get(i + 1) {
                        let next_trimmed = next.trim();
                        if !next_trimmed.is_empty() && next_trimmed.len() <= 40 {
                            return Some(next_trimmed.to_string());
                        }
                    }
                }
            }
        }
        None
    }

    fn detect_currency(&self, text: &str) -> Option<Currency> {
        let lower = text.to_lowercase();
        if text.contains('$') || lower.contains("usd") {
            Some(Currency::USD)
        } else if text.contains('€') || lower.contains("eur") {
            Some(Currency::EUR)
        } else if text.contains('£') || lower.contains("gbp") {
            Some(Currency::GBP)
        } else if lower.contains("jpy") || lower.contains("yen") {
            Some(Currency::JPY)
        } else {
            None
        }
    }
}

impl Default for InvoiceRuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_date(s: &str) -> Option<NaiveDate> {
    let formats = [
        "%m/%d/%Y", "%m/%d/%y", "%d/%m/%Y", "%d/%m/%y",
        "%m-%d-%Y", "%m-%d-%y", "%Y-%m-%d", "%Y/%m/%d",
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

    if cleaned.is_empty() {
        return None;
    }

    let cleaned = if cleaned.starts_with('(') && cleaned.ends_with(')') {
        format!("-{}", &cleaned[1..cleaned.len() - 1])
    } else {
        cleaned
    };

    cleaned.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_invoice_number() {
        let engine = InvoiceRuleEngine::new();

        let text = "Acme Corp\nInvoice #: INV-2024-001\nDate: 01/15/2024";
        let result = engine.extract(text);
        assert_eq!(result.invoice_number, Some("INV-2024-001".to_string()));
    }

    #[test]
    fn test_extract_invoice_number_next_line() {
        let engine = InvoiceRuleEngine::new();

        let text = "Invoice Number:\nINV-9999\nDate: 01/15/2024";
        let result = engine.extract(text);
        assert_eq!(result.invoice_number, Some("INV-9999".to_string()));
    }

    #[test]
    fn test_extract_dates() {
        let engine = InvoiceRuleEngine::new();

        let text = "Invoice Date: 01/15/2024\nDue Date: 02/15/2024\nTotal: $100.00";
        let result = engine.extract(text);
        assert_eq!(result.invoice_date, Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()));
        assert_eq!(result.due_date, Some(NaiveDate::from_ymd_opt(2024, 2, 15).unwrap()));
    }

    #[test]
    fn test_extract_amounts() {
        let engine = InvoiceRuleEngine::new();

        let text = "Subtotal: $1,000.00\nTax: $80.00\nTotal: $1,080.00";
        let result = engine.extract(text);
        assert_eq!(result.subtotal, Some(Decimal::new(100000, 2)));
        assert_eq!(result.tax_amount, Some(Decimal::new(8000, 2)));
        assert_eq!(result.total_amount, Some(Decimal::new(108000, 2)));
    }

    #[test]
    fn test_extract_vendor_labeled() {
        let engine = InvoiceRuleEngine::new();

        let text = "From: Acme Corporation\nBill To: Widget Inc\nInvoice #: 123";
        let result = engine.extract(text);
        assert_eq!(result.vendor, Some("Acme Corporation".to_string()));
        assert_eq!(result.bill_to, Some("Widget Inc".to_string()));
    }

    #[test]
    fn test_extract_vendor_header_heuristic() {
        let engine = InvoiceRuleEngine::new();

        let text = "Acme Corporation\n123 Main St\nInvoice #: 456\nDate: 01/01/2024";
        let result = engine.extract(text);
        assert_eq!(result.vendor, Some("Acme Corporation".to_string()));
    }

    #[test]
    fn test_extract_po_number() {
        let engine = InvoiceRuleEngine::new();

        let text = "Invoice #: 123\nPO #: PO-5678\nTotal: $100";
        let result = engine.extract(text);
        assert_eq!(result.po_number, Some("PO-5678".to_string()));
    }

    #[test]
    fn test_detect_currency() {
        let engine = InvoiceRuleEngine::new();

        let text = "Total: €1,234.56";
        let result = engine.extract(text);
        assert_eq!(result.currency, Some(Currency::EUR));
    }

    #[test]
    fn test_empty_text() {
        let engine = InvoiceRuleEngine::new();
        let result = engine.extract("");
        assert!(result.invoice_number.is_none());
        assert!(result.total_amount.is_none());
        assert!(result.vendor.is_none());
    }

    #[test]
    fn test_total_not_confused_with_subtotal() {
        let engine = InvoiceRuleEngine::new();

        let text = "Subtotal: $500.00\nTotal: $550.00";
        let result = engine.extract(text);
        assert_eq!(result.subtotal, Some(Decimal::new(50000, 2)));
        assert_eq!(result.total_amount, Some(Decimal::new(55000, 2)));
    }
}
