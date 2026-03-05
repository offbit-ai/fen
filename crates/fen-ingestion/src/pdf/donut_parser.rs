//! Donut JSON output → Invoice mapping.
//!
//! Donut (CORD-v2 fine-tuned) outputs structured JSON with receipt/invoice fields:
//! ```json
//! {
//!   "menu": [{"nm": "item", "price": "10.00", "cnt": "1"}],
//!   "sub_total": {"subtotal_price": "10.00"},
//!   "total": {"total_price": "10.00", "cashprice": "10.00"},
//!   "void_menu": []
//! }
//! ```
//!
//! This parser maps those keys to `Invoice` fields where possible, filling
//! in defaults for fields Donut doesn't extract (vendor, bill_to, etc.).

use chrono::Utc;
use fen_core::domain::{DocumentId, Invoice, LineItem};
use rust_decimal::Decimal;
use std::str::FromStr;

use crate::error::IngestionError;

/// Parse Donut's JSON output into an Invoice.
///
/// This is a best-effort mapping — Donut (CORD model) is trained on receipts
/// and may not extract all invoice fields. Fields not found default to
/// UNKNOWN / zero values.
pub fn parse_donut_output(
    json: &serde_json::Value,
    document_id: DocumentId,
    raw_text: &str,
) -> Result<Invoice, IngestionError> {
    let today = Utc::now().date_naive();
    let mut invoice = Invoice::new(
        extract_invoice_number(json).unwrap_or_else(|| format!("DONUT-{}", document_id)),
        today,
    );
    invoice.document_id = document_id;
    invoice.extracted_text = raw_text.to_string();

    // Total amount
    if let Some(total) = extract_total(json) {
        invoice.total_amount = total;
    }

    // Subtotal
    if let Some(subtotal) = extract_subtotal(json) {
        invoice.subtotal = subtotal;
    }

    // Tax
    if let Some(tax) = extract_tax(json) {
        invoice.tax_amount = tax;
    }

    // Line items from "menu"
    if let Some(items) = json.get("menu").and_then(|m| m.as_array()) {
        for item in items {
            if let Some(line_item) = parse_menu_item(item) {
                invoice.line_items.push(line_item);
            }
        }
    }

    // Confidence is moderate for Donut — it's a fallback path
    invoice.confidence_score = 0.5;

    Ok(invoice)
}

/// Merge Donut results into an existing invoice, filling only UNKNOWN/zero fields.
pub fn merge_donut_into_invoice(invoice: &mut Invoice, donut_json: &serde_json::Value) {
    // Only fill total if primary didn't extract it
    if invoice.total_amount.is_zero() {
        if let Some(total) = extract_total(donut_json) {
            invoice.total_amount = total;
        }
    }

    if invoice.subtotal.is_zero() {
        if let Some(subtotal) = extract_subtotal(donut_json) {
            invoice.subtotal = subtotal;
        }
    }

    if invoice.tax_amount.is_zero() {
        if let Some(tax) = extract_tax(donut_json) {
            invoice.tax_amount = tax;
        }
    }

    // Add line items if primary didn't extract any
    if invoice.line_items.is_empty() {
        if let Some(items) = donut_json.get("menu").and_then(|m| m.as_array()) {
            for item in items {
                if let Some(line_item) = parse_menu_item(item) {
                    invoice.line_items.push(line_item);
                }
            }
        }
    }
}

fn extract_invoice_number(json: &serde_json::Value) -> Option<String> {
    // CORD format doesn't have a standard invoice number field,
    // but some fine-tuned variants do
    json.get("invoice_number")
        .or_else(|| json.get("receipt_no"))
        .or_else(|| json.get("order_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn extract_total(json: &serde_json::Value) -> Option<Decimal> {
    json.get("total")
        .and_then(|t| {
            t.get("total_price")
                .or_else(|| t.get("cashprice"))
                .or_else(|| t.get("total_etc"))
                .and_then(|v| v.as_str())
        })
        .and_then(parse_decimal)
}

fn extract_subtotal(json: &serde_json::Value) -> Option<Decimal> {
    json.get("sub_total")
        .and_then(|t| t.get("subtotal_price").and_then(|v| v.as_str()))
        .and_then(parse_decimal)
}

fn extract_tax(json: &serde_json::Value) -> Option<Decimal> {
    json.get("total")
        .and_then(|t| t.get("tax_price").and_then(|v| v.as_str()))
        .and_then(parse_decimal)
}

fn parse_menu_item(item: &serde_json::Value) -> Option<LineItem> {
    let name = item.get("nm").and_then(|v| v.as_str()).unwrap_or("Item");
    let price = item
        .get("price")
        .and_then(|v| v.as_str())
        .and_then(parse_decimal)
        .unwrap_or(Decimal::ZERO);
    let qty = item
        .get("cnt")
        .and_then(|v| v.as_str())
        .and_then(parse_decimal)
        .unwrap_or(Decimal::ONE);

    Some(LineItem {
        line_number: 0,
        item_code: None,
        description: name.to_string(),
        quantity: qty,
        unit: None,
        unit_price: price,
        tax_rate: None,
        discount: None,
        total: price * qty,
    })
}

fn parse_decimal(s: &str) -> Option<Decimal> {
    // Strip currency symbols and whitespace
    let cleaned: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    Decimal::from_str(&cleaned).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_donut_output() {
        let donut_json = json!({
            "menu": [
                {"nm": "Coffee", "price": "3.50", "cnt": "2"},
                {"nm": "Muffin", "price": "2.00", "cnt": "1"}
            ],
            "sub_total": {"subtotal_price": "9.00"},
            "total": {"total_price": "9.50", "tax_price": "0.50"}
        });

        let doc_id = DocumentId::new();
        let invoice = parse_donut_output(&donut_json, doc_id, "raw text").unwrap();

        assert_eq!(invoice.line_items.len(), 2);
        assert_eq!(invoice.line_items[0].description, "Coffee");
        assert_eq!(invoice.line_items[0].quantity, Decimal::new(2, 0));
        assert_eq!(invoice.line_items[0].unit_price, Decimal::new(350, 2));
        assert_eq!(invoice.total_amount, Decimal::new(950, 2));
        assert_eq!(invoice.subtotal, Decimal::new(900, 2));
        assert_eq!(invoice.tax_amount, Decimal::new(50, 2));
    }

    #[test]
    fn test_merge_donut_into_invoice() {
        let mut invoice = Invoice::new("INV-001", Utc::now().date_naive());

        let donut_json = json!({
            "total": {"total_price": "100.00"},
            "menu": [{"nm": "Widget", "price": "100.00", "cnt": "1"}]
        });

        merge_donut_into_invoice(&mut invoice, &donut_json);

        assert_eq!(invoice.total_amount, Decimal::new(10000, 2));
        assert_eq!(invoice.line_items.len(), 1);
    }

    #[test]
    fn test_merge_does_not_overwrite() {
        let mut invoice = Invoice::new("INV-001", Utc::now().date_naive());
        invoice.total_amount = Decimal::new(5000, 2);

        let donut_json = json!({
            "total": {"total_price": "999.99"}
        });

        merge_donut_into_invoice(&mut invoice, &donut_json);

        // Should NOT overwrite existing total
        assert_eq!(invoice.total_amount, Decimal::new(5000, 2));
    }

    #[test]
    fn test_parse_decimal() {
        assert_eq!(parse_decimal("$10.50"), Some(Decimal::new(1050, 2)));
        assert_eq!(parse_decimal("100"), Some(Decimal::new(100, 0)));
        assert_eq!(parse_decimal(""), None);
    }
}
