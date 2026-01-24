use chrono::NaiveDate;
use rust_decimal::Decimal;

use fen_core::domain::{Currency, DocumentId, Invoice, InvoiceId, LineItem, Party, TenantId};
use fen_core::ValidationStatus;
use fen_ml::layout::{EntityType, LayoutResult, NamedEntity};
use fen_ml::table::ExtractedTable;
use fen_ml::ProcessedDocument;

use crate::error::IngestionError;

/// ML-enhanced invoice parser using LayoutLMv3 and table extraction
pub struct MlInvoiceParser {
    /// Fallback to regex for certain fields
    regex_parser: super::InvoiceParser,
}

impl MlInvoiceParser {
    pub fn new() -> Self {
        Self {
            regex_parser: super::InvoiceParser::new(),
        }
    }

    /// Parse invoice from ML-processed document
    pub fn parse(
        &self,
        processed: &ProcessedDocument,
        document_id: DocumentId,
    ) -> Result<Invoice, IngestionError> {
        let layout = &processed.layout_result;
        let tables = &processed.tables;
        let text = &processed.text;

        // Extract structured data from ML results
        let invoice_number = self.extract_invoice_number(layout, text);
        let invoice_date = self.extract_date(layout, text);
        let due_date = self.extract_due_date(layout, text);
        let total_amount = self.extract_total_amount(layout, text);
        let subtotal = self.extract_subtotal(layout, text);
        let tax_amount = self.extract_tax_amount(layout, text);
        let vendor = self.extract_vendor(layout, text);
        let bill_to = self.extract_bill_to(layout, text);
        let po_number = self.extract_po_number(layout, text);
        let currency = self.detect_currency(text);
        let line_items = self.extract_line_items(tables, layout, text);

        // Calculate confidence based on ML results
        let confidence_score = self.calculate_confidence(layout, tables);

        Ok(Invoice {
            id: InvoiceId::new(),
            document_id,
            tenant_id: TenantId::system(),
            invoice_number,
            invoice_date,
            due_date,
            po_number,
            vendor,
            bill_to,
            currency,
            line_items,
            subtotal,
            tax_amount,
            discount_amount: Decimal::ZERO,
            total_amount,
            validation_status: ValidationStatus::Pending,
            confidence_score,
            extracted_text: text.clone(),
            contract_id: None,
            contract_number: None,
        })
    }

    fn extract_invoice_number(&self, layout: &LayoutResult, text: &str) -> String {
        // First try ML-extracted entities
        for entity in &layout.entities {
            if entity.entity_type == EntityType::InvoiceNumber {
                return entity.value.clone();
            }
        }

        // Look in key-value pairs
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("invoice")
                && (key_lower.contains("no")
                    || key_lower.contains("#")
                    || key_lower.contains("number"))
            {
                return kv.value.clone();
            }
        }

        // Fallback to regex
        self.regex_parser
            .parse(text, DocumentId::new())
            .map(|inv| inv.invoice_number)
            .unwrap_or_else(|_| {
                format!(
                    "UNKNOWN-{}",
                    uuid::Uuid::new_v4().to_string().split('-').next().unwrap()
                )
            })
    }

    fn extract_date(&self, layout: &LayoutResult, _text: &str) -> NaiveDate {
        // First try ML-extracted entities
        let date_entities: Vec<&NamedEntity> = layout
            .entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Date)
            .collect();

        // Look for invoice date in key-value pairs
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("invoice") && key_lower.contains("date") {
                if let Some(date) = parse_date(&kv.value) {
                    return date;
                }
            }
            if key_lower == "date" && !key_lower.contains("due") {
                if let Some(date) = parse_date(&kv.value) {
                    return date;
                }
            }
        }

        // Use first date entity that looks like invoice date
        for entity in &date_entities {
            if let Some(date) = parse_date(&entity.value) {
                return date;
            }
        }

        // Fallback
        chrono::Local::now().date_naive()
    }

    fn extract_due_date(&self, layout: &LayoutResult, _text: &str) -> Option<NaiveDate> {
        // Look for due date in key-value pairs
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("due") && key_lower.contains("date") {
                if let Some(date) = parse_date(&kv.value) {
                    return Some(date);
                }
            }
            if key_lower.contains("payment") && key_lower.contains("date") {
                if let Some(date) = parse_date(&kv.value) {
                    return Some(date);
                }
            }
        }
        None
    }

    fn extract_total_amount(&self, layout: &LayoutResult, text: &str) -> Decimal {
        // Look for total in key-value pairs
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("total") && !key_lower.contains("sub") {
                if let Some(amount) = parse_amount(&kv.value) {
                    return amount;
                }
            }
            if key_lower.contains("amount due") || key_lower.contains("balance due") {
                if let Some(amount) = parse_amount(&kv.value) {
                    return amount;
                }
            }
        }

        // Look for amount entities
        let amounts: Vec<Decimal> = layout
            .entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Amount)
            .filter_map(|e| parse_amount(&e.value))
            .collect();

        if let Some(&max_amount) = amounts.iter().max() {
            return max_amount;
        }

        // Fallback to regex
        self.regex_parser
            .parse(text, DocumentId::new())
            .map(|inv| inv.total_amount)
            .unwrap_or(Decimal::ZERO)
    }

    fn extract_subtotal(&self, layout: &LayoutResult, _text: &str) -> Decimal {
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("subtotal") || key_lower.contains("sub total") {
                if let Some(amount) = parse_amount(&kv.value) {
                    return amount;
                }
            }
        }
        Decimal::ZERO
    }

    fn extract_tax_amount(&self, layout: &LayoutResult, _text: &str) -> Decimal {
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("tax") || key_lower.contains("vat") || key_lower.contains("gst") {
                if let Some(amount) = parse_amount(&kv.value) {
                    return amount;
                }
            }
        }
        Decimal::ZERO
    }

    fn extract_vendor(&self, layout: &LayoutResult, text: &str) -> Party {
        // Look for organization entities
        for entity in &layout.entities {
            if entity.entity_type == EntityType::Organization {
                return Party::new(entity.value.clone());
            }
        }

        // Look in key-value pairs
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("from")
                || key_lower.contains("vendor")
                || key_lower.contains("supplier")
                || key_lower.contains("seller")
            {
                return Party::new(kv.value.clone());
            }
        }

        // Fallback to regex
        self.regex_parser
            .parse(text, DocumentId::new())
            .map(|inv| inv.vendor)
            .unwrap_or_else(|_| Party::unknown())
    }

    fn extract_bill_to(&self, layout: &LayoutResult, _text: &str) -> Party {
        // Look in key-value pairs
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("bill to")
                || key_lower.contains("customer")
                || key_lower.contains("client")
                || key_lower.contains("buyer")
            {
                return Party::new(kv.value.clone());
            }
        }
        Party::unknown()
    }

    fn extract_po_number(&self, layout: &LayoutResult, text: &str) -> Option<String> {
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("po") || key_lower.contains("purchase order") {
                return Some(kv.value.clone());
            }
        }

        // Fallback
        self.regex_parser
            .parse(text, DocumentId::new())
            .ok()
            .and_then(|inv| inv.po_number)
    }

    fn detect_currency(&self, text: &str) -> Currency {
        let text_lower = text.to_lowercase();

        if text.contains('$') || text_lower.contains("usd") {
            Currency::USD
        } else if text.contains('€') || text_lower.contains("eur") {
            Currency::EUR
        } else if text.contains('£') || text_lower.contains("gbp") {
            Currency::GBP
        } else if text_lower.contains("jpy") || text_lower.contains("yen") {
            Currency::JPY
        } else {
            Currency::USD // Default
        }
    }

    fn extract_line_items(
        &self,
        tables: &[ExtractedTable],
        _layout: &LayoutResult,
        text: &str,
    ) -> Vec<LineItem> {
        let mut items = Vec::new();

        // Try to extract from tables first
        for table in tables {
            if self.is_line_item_table(table) {
                items.extend(self.extract_items_from_table(table));
            }
        }

        if !items.is_empty() {
            return items;
        }

        // Fallback to regex
        self.regex_parser
            .parse(text, DocumentId::new())
            .map(|inv| inv.line_items)
            .unwrap_or_default()
    }

    fn is_line_item_table(&self, table: &ExtractedTable) -> bool {
        if table.cells.is_empty() {
            return false;
        }

        // Check if header row contains expected column names
        let header_row = &table.cells[0];
        let header_text: String = header_row
            .iter()
            .map(|c| c.text.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");

        // Look for typical line item table headers
        let has_description = header_text.contains("description")
            || header_text.contains("item")
            || header_text.contains("product");
        let has_quantity = header_text.contains("qty") || header_text.contains("quantity");
        let has_amount = header_text.contains("amount")
            || header_text.contains("total")
            || header_text.contains("price");

        has_description && (has_quantity || has_amount)
    }

    fn extract_items_from_table(&self, table: &ExtractedTable) -> Vec<LineItem> {
        let mut items = Vec::new();

        if table.cells.len() < 2 {
            return items;
        }

        // Find column indices
        let header_row = &table.cells[0];
        let mut desc_col: Option<usize> = None;
        let mut qty_col: Option<usize> = None;
        let mut unit_price_col: Option<usize> = None;
        let mut total_col: Option<usize> = None;

        for (i, cell) in header_row.iter().enumerate() {
            let lower = cell.text.to_lowercase();
            if lower.contains("description") || lower.contains("item") || lower.contains("product")
            {
                desc_col = Some(i);
            } else if lower.contains("qty") || lower.contains("quantity") {
                qty_col = Some(i);
            } else if lower.contains("unit") && lower.contains("price") {
                unit_price_col = Some(i);
            } else if lower.contains("amount") || lower.contains("total") {
                total_col = Some(i);
            } else if lower.contains("price") && unit_price_col.is_none() {
                unit_price_col = Some(i);
            }
        }

        // Extract data rows
        for (row_idx, row) in table.cells.iter().skip(1).enumerate() {
            let description = desc_col
                .and_then(|i| row.get(i))
                .map(|c| c.text.trim().to_string())
                .unwrap_or_default();

            let quantity = qty_col
                .and_then(|i| row.get(i))
                .and_then(|c| c.text.trim().parse::<Decimal>().ok())
                .unwrap_or(Decimal::ONE);

            let unit_price = unit_price_col
                .and_then(|i| row.get(i))
                .and_then(|c| parse_amount(&c.text))
                .unwrap_or(Decimal::ZERO);

            let total = total_col
                .and_then(|i| row.get(i))
                .and_then(|c| parse_amount(&c.text))
                .unwrap_or_else(|| quantity * unit_price);

            if !description.is_empty() && (total > Decimal::ZERO || unit_price > Decimal::ZERO) {
                items.push(LineItem::new(
                    (row_idx + 1) as u32,
                    &description,
                    quantity,
                    total,
                ));
            }
        }

        items
    }

    fn calculate_confidence(&self, layout: &LayoutResult, tables: &[ExtractedTable]) -> f32 {
        let mut score = 0.0;
        let mut factors = 0;

        // Factor in OCR confidence if available
        if layout.processing_time_ms > 0 {
            // We have ML results, start with higher base confidence
            score += 0.7;
            factors += 1;
        }

        // Factor in entity extraction confidence
        if !layout.entities.is_empty() {
            let avg_entity_conf: f32 = layout.entities.iter().map(|e| e.confidence).sum::<f32>()
                / layout.entities.len() as f32;
            score += avg_entity_conf;
            factors += 1;
        }

        // Factor in key-value pair extraction
        if !layout.key_value_pairs.is_empty() {
            let avg_kv_conf: f32 = layout
                .key_value_pairs
                .iter()
                .map(|kv| kv.confidence)
                .sum::<f32>()
                / layout.key_value_pairs.len() as f32;
            score += avg_kv_conf;
            factors += 1;
        }

        // Factor in table extraction
        if !tables.is_empty() {
            let avg_table_conf: f32 =
                tables.iter().map(|t| t.confidence).sum::<f32>() / tables.len() as f32;
            score += avg_table_conf;
            factors += 1;
        }

        if factors > 0 {
            score / factors as f32
        } else {
            0.3 // Low confidence when no ML results available
        }
    }
}

impl Default for MlInvoiceParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse various date formats
fn parse_date(s: &str) -> Option<NaiveDate> {
    let formats = [
        "%m/%d/%Y", "%m/%d/%y", "%d/%m/%Y", "%d/%m/%y", "%m-%d-%Y", "%m-%d-%y", "%Y-%m-%d",
        "%Y/%m/%d",
    ];

    // Remove currency symbols and extra whitespace
    let cleaned = s.trim().replace(['$', '€', '£'], "");

    for fmt in formats {
        if let Ok(date) = NaiveDate::parse_from_str(&cleaned, fmt) {
            return Some(date);
        }
    }

    // Try parsing "Month Day, Year" format
    let month_formats = ["%B %d, %Y", "%b %d, %Y", "%B %d %Y", "%b %d %Y"];
    for fmt in month_formats {
        if let Ok(date) = NaiveDate::parse_from_str(&cleaned, fmt) {
            return Some(date);
        }
    }

    None
}

/// Parse currency amount
fn parse_amount(s: &str) -> Option<Decimal> {
    // Remove currency symbols and spaces
    let cleaned = s
        .replace(['$', '€', '£', '¥', ',', ' '], "")
        .trim()
        .to_string();

    // Handle negative values in parentheses (accounting format)
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
    fn test_parse_amount_with_symbols() {
        assert_eq!(parse_amount("$1,234.56"), Some(Decimal::new(123456, 2)));
        assert_eq!(parse_amount("€100.00"), Some(Decimal::new(10000, 2)));
        assert_eq!(parse_amount("£50"), Some(Decimal::new(50, 0)));
    }

    #[test]
    fn test_parse_amount_negative() {
        assert_eq!(parse_amount("(100.00)"), Some(Decimal::new(-10000, 2)));
    }

    #[test]
    fn test_parse_date_formats() {
        assert!(parse_date("01/15/2024").is_some());
        assert!(parse_date("2024-01-15").is_some());
        assert!(parse_date("January 15, 2024").is_some());
    }
}
