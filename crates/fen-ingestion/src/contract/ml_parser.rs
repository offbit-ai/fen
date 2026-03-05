use chrono::NaiveDate;
use rust_decimal::Decimal;

use fen_core::domain::{
    ClauseType, Contract, ContractClause, ContractId, ContractType, Currency, DocumentId, Party,
    TenantId,
};
use fen_core::ValidationStatus;
use fen_ml::layout::{EntityType, LayoutLabel, LayoutResult, NamedEntity};
use fen_ml::table::ExtractedTable;
use fen_ml::ProcessedDocument;

use crate::error::IngestionError;

/// ML-enhanced contract parser using LayoutLMv3 and table extraction
pub struct MlContractParser {
    /// Fallback to regex for fields ML can't extract
    regex_parser: super::ContractParser,
}

impl MlContractParser {
    pub fn new() -> Self {
        Self {
            regex_parser: super::ContractParser::new(),
        }
    }

    /// Parse contract from ML-processed document
    pub fn parse(
        &self,
        processed: &ProcessedDocument,
        document_id: DocumentId,
    ) -> Result<Contract, IngestionError> {
        let layout = &processed.layout_result;
        let tables = &processed.tables;
        let text = &processed.text;

        // Extract structured data from ML results
        let contract_number = self.extract_contract_number(layout, text);
        let title = self.extract_title(layout, text);
        let contract_type = self.detect_contract_type(layout, text);
        let parties = self.extract_parties(layout, text);
        let effective_date = self
            .extract_effective_date(layout, text)
            .unwrap_or_else(|| chrono::Local::now().date_naive());
        let expiration_date = self.extract_expiration_date(layout, text);
        let execution_date = self.extract_execution_date(layout, text);
        let total_value = self.extract_total_value(layout, tables, text);
        let currency = self.detect_currency(text);
        let clauses = self.extract_clauses(layout, text);

        // Calculate confidence based on ML results
        let confidence_score = self.calculate_confidence(layout, tables);

        Ok(Contract {
            id: ContractId::new(),
            document_id,
            tenant_id: TenantId::system(),
            contract_number,
            title,
            contract_type,
            parties,
            effective_date,
            expiration_date,
            execution_date,
            total_value,
            currency: Some(currency),
            clauses,
            validation_status: ValidationStatus::Pending,
            confidence_score,
            extracted_text: text.clone(),
        })
    }

    fn extract_contract_number(&self, layout: &LayoutResult, text: &str) -> Option<String> {
        // First try ML-extracted entities
        for entity in &layout.entities {
            if entity.entity_type == EntityType::ContractNumber {
                return Some(entity.value.clone());
            }
        }

        // Look in key-value pairs
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if (key_lower.contains("contract") || key_lower.contains("agreement"))
                && (key_lower.contains("no")
                    || key_lower.contains("#")
                    || key_lower.contains("number"))
            {
                return Some(kv.value.clone());
            }
            if key_lower.contains("reference") && key_lower.contains("no") {
                return Some(kv.value.clone());
            }
        }

        // Fallback to regex
        self.regex_parser
            .parse(text, DocumentId::new())
            .ok()
            .and_then(|c| c.contract_number)
    }

    fn extract_title(&self, layout: &LayoutResult, text: &str) -> String {
        // Look for title in layout regions
        for region in &layout.regions {
            if region.label == LayoutLabel::Title && !region.text.is_empty() {
                return region.text.trim().to_string();
            }
        }

        // Look for title in key-value pairs
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower == "title"
                || key_lower == "agreement title"
                || key_lower == "contract title"
            {
                return kv.value.clone();
            }
        }

        // Look for header regions that might contain the title
        for region in &layout.regions {
            if region.label == LayoutLabel::Header && !region.text.is_empty() {
                let trimmed = region.text.trim();
                // Header that looks like a contract title
                if trimmed.len() > 5 && trimmed.len() < 200 {
                    return trimmed.to_string();
                }
            }
        }

        // Fallback to regex
        self.regex_parser
            .parse(text, DocumentId::new())
            .map(|c| c.title)
            .unwrap_or_else(|_| "Untitled Contract".to_string())
    }

    fn detect_contract_type(&self, layout: &LayoutResult, text: &str) -> ContractType {
        // Check title region and KV pairs for contract type indicators
        let title_text = layout
            .regions
            .iter()
            .filter(|r| r.label == LayoutLabel::Title || r.label == LayoutLabel::Header)
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let combined = format!("{} {}", title_text, text);
        let combined_lower = combined.to_lowercase();

        // Check for specific contract type patterns in priority order
        if combined_lower.contains("master service agreement") || combined_lower.contains("msa") {
            ContractType::MasterServiceAgreement
        } else if combined_lower.contains("statement of work") || combined_lower.contains("sow") {
            ContractType::StatementOfWork
        } else if combined_lower.contains("purchase order") {
            ContractType::PurchaseOrder
        } else if combined_lower.contains("amendment") {
            ContractType::Amendment
        } else if combined_lower.contains("service agreement")
            || combined_lower.contains("service contract")
        {
            ContractType::ServiceAgreement
        } else {
            // Look in KV pairs
            for kv in &layout.key_value_pairs {
                let key_lower = kv.key.to_lowercase();
                if key_lower.contains("type") || key_lower.contains("category") {
                    let val_lower = kv.value.to_lowercase();
                    if val_lower.contains("master") || val_lower.contains("msa") {
                        return ContractType::MasterServiceAgreement;
                    }
                    if val_lower.contains("purchase") {
                        return ContractType::PurchaseOrder;
                    }
                    if val_lower.contains("statement") || val_lower.contains("sow") {
                        return ContractType::StatementOfWork;
                    }
                    if val_lower.contains("amendment") {
                        return ContractType::Amendment;
                    }
                    if val_lower.contains("service") {
                        return ContractType::ServiceAgreement;
                    }
                }
            }

            // Fallback to regex
            self.regex_parser
                .parse(text, DocumentId::new())
                .map(|c| c.contract_type)
                .unwrap_or(ContractType::Other)
        }
    }

    fn extract_parties(&self, layout: &LayoutResult, text: &str) -> Vec<Party> {
        let mut parties = Vec::new();

        // Collect organization entities from ML
        for entity in &layout.entities {
            if entity.entity_type == EntityType::Organization && entity.confidence > 0.5 {
                let name = entity.value.trim();
                if !name.is_empty() && !parties.iter().any(|p: &Party| p.name == name) {
                    parties.push(Party::new(name));
                }
            }
        }

        // Also check KV pairs for party information
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("party")
                || key_lower.contains("client")
                || key_lower.contains("vendor")
                || key_lower.contains("provider")
                || key_lower.contains("contractor")
                || key_lower.contains("buyer")
                || key_lower.contains("seller")
                || key_lower.contains("supplier")
                || key_lower.contains("company")
            {
                let name = kv.value.trim();
                if !name.is_empty() && !parties.iter().any(|p| p.name == name) {
                    parties.push(Party::new(name));
                }
            }
        }

        // Enrich parties with address/contact from ML entities
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("address") && !parties.is_empty() {
                // Associate with first party that has no address
                if let Some(party) = parties.iter_mut().find(|p| p.address.is_none()) {
                    party.address = Some(fen_core::domain::party::Address {
                        street: Some(kv.value.clone()),
                        ..Default::default()
                    });
                }
            }
            if key_lower.contains("email") && !parties.is_empty() {
                if let Some(party) = parties.iter_mut().find(|p| p.contact.is_none()) {
                    party.contact = Some(fen_core::domain::party::Contact {
                        email: Some(kv.value.clone()),
                        ..Default::default()
                    });
                }
            }
        }

        if parties.is_empty() {
            // Fallback to regex
            let regex_parties = self
                .regex_parser
                .parse(text, DocumentId::new())
                .map(|c| c.parties)
                .unwrap_or_else(|_| vec![Party::unknown()]);
            return regex_parties;
        }

        parties
    }

    fn extract_effective_date(&self, layout: &LayoutResult, text: &str) -> Option<NaiveDate> {
        // Check KV pairs for effective/start/commencement date
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("effective")
                || key_lower.contains("commencement")
                || key_lower.contains("start date")
            {
                if let Some(date) = parse_date(&kv.value) {
                    return Some(date);
                }
            }
        }

        // Look at date entities - the first date in a contract is often the effective date
        let date_entities: Vec<&NamedEntity> = layout
            .entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Date)
            .collect();

        if let Some(first_date) = date_entities.first() {
            if let Some(date) = parse_date(&first_date.value) {
                return Some(date);
            }
        }

        // Fallback to regex
        self.regex_parser
            .parse(text, DocumentId::new())
            .ok()
            .map(|c| c.effective_date)
    }

    fn extract_expiration_date(&self, layout: &LayoutResult, text: &str) -> Option<NaiveDate> {
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("expir")
                || key_lower.contains("end date")
                || key_lower.contains("termination date")
                || key_lower.contains("term end")
            {
                if let Some(date) = parse_date(&kv.value) {
                    return Some(date);
                }
            }
        }

        // Fallback to regex
        self.regex_parser
            .parse(text, DocumentId::new())
            .ok()
            .and_then(|c| c.expiration_date)
    }

    fn extract_execution_date(&self, layout: &LayoutResult, text: &str) -> Option<NaiveDate> {
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("execution")
                || key_lower.contains("signed")
                || key_lower.contains("date of execution")
            {
                if let Some(date) = parse_date(&kv.value) {
                    return Some(date);
                }
            }
        }

        // Look near signature regions for dates
        for region in &layout.regions {
            if region.label == LayoutLabel::Signature {
                // Check entities near the signature region for dates
                for entity in &layout.entities {
                    if entity.entity_type == EntityType::Date {
                        if let Some(date) = parse_date(&entity.value) {
                            return Some(date);
                        }
                    }
                }
            }
        }

        // Fallback to regex
        self.regex_parser
            .parse(text, DocumentId::new())
            .ok()
            .and_then(|c| c.execution_date)
    }

    fn extract_total_value(
        &self,
        layout: &LayoutResult,
        tables: &[ExtractedTable],
        text: &str,
    ) -> Option<Decimal> {
        // Check KV pairs for contract value
        for kv in &layout.key_value_pairs {
            let key_lower = kv.key.to_lowercase();
            if key_lower.contains("total")
                || key_lower.contains("value")
                || key_lower.contains("not to exceed")
                || key_lower.contains("maximum")
                || key_lower.contains("contract amount")
                || key_lower.contains("compensation")
            {
                if let Some(amount) = parse_amount(&kv.value) {
                    return Some(amount);
                }
            }
        }

        // Check amount entities
        let amounts: Vec<Decimal> = layout
            .entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Amount)
            .filter_map(|e| parse_amount(&e.value))
            .collect();

        // For contracts, take the largest amount as contract value
        if let Some(&max_amount) = amounts.iter().max() {
            return Some(max_amount);
        }

        // Check tables for pricing/value tables
        for table in tables {
            if self.is_pricing_table(table) {
                if let Some(total) = self.extract_total_from_table(table) {
                    return Some(total);
                }
            }
        }

        // Fallback to regex
        self.regex_parser
            .parse(text, DocumentId::new())
            .ok()
            .and_then(|c| c.total_value)
    }

    fn is_pricing_table(&self, table: &ExtractedTable) -> bool {
        if table.cells.is_empty() {
            return false;
        }

        let header_row = &table.cells[0];
        let header_text: String = header_row
            .iter()
            .map(|c| c.text.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");

        header_text.contains("amount")
            || header_text.contains("price")
            || header_text.contains("value")
            || header_text.contains("cost")
            || header_text.contains("rate")
            || header_text.contains("fee")
    }

    fn extract_total_from_table(&self, table: &ExtractedTable) -> Option<Decimal> {
        // Look for a "total" row in the table
        for row in &table.cells {
            for (i, cell) in row.iter().enumerate() {
                if cell.text.to_lowercase().contains("total") {
                    // Look for amount in same row, next columns
                    for next_cell in row.iter().skip(i + 1) {
                        if let Some(amount) = parse_amount(&next_cell.text) {
                            return Some(amount);
                        }
                    }
                }
            }
        }

        // If no total row, sum the amounts in the last column
        if table.cells.len() > 1 {
            let last_col = table.cells[0].len().saturating_sub(1);
            let sum: Decimal = table
                .cells
                .iter()
                .skip(1) // Skip header
                .filter_map(|row| row.get(last_col))
                .filter_map(|cell| parse_amount(&cell.text))
                .sum();

            if sum > Decimal::ZERO {
                return Some(sum);
            }
        }

        None
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

    fn extract_clauses(&self, layout: &LayoutResult, text: &str) -> Vec<ContractClause> {
        let mut clauses = Vec::new();

        // Use layout regions to identify clause sections
        // Contracts typically have numbered sections with headers
        let mut current_clause_type: Option<ClauseType> = None;
        let mut current_text = String::new();

        for region in &layout.regions {
            match region.label {
                LayoutLabel::Title | LayoutLabel::Header => {
                    // Save previous clause
                    if let Some(clause_type) = current_clause_type.take() {
                        let trimmed = current_text.trim();
                        if trimmed.len() >= 20 {
                            clauses.push(ContractClause::new(clause_type, trimmed));
                        }
                        current_text.clear();
                    }

                    // Detect clause type from header
                    current_clause_type = detect_clause_type(&region.text);
                }
                LayoutLabel::Text | LayoutLabel::List => {
                    if current_clause_type.is_some() {
                        current_text.push_str(&region.text);
                        current_text.push('\n');
                    }
                }
                _ => {}
            }
        }

        // Save last clause
        if let Some(clause_type) = current_clause_type {
            let trimmed = current_text.trim();
            if trimmed.len() >= 20 {
                clauses.push(ContractClause::new(clause_type, trimmed));
            }
        }

        // If ML layout didn't yield clauses, fallback to regex
        if clauses.is_empty() {
            return self
                .regex_parser
                .parse(text, DocumentId::new())
                .map(|c| c.clauses)
                .unwrap_or_default();
        }

        clauses
    }

    fn calculate_confidence(&self, layout: &LayoutResult, tables: &[ExtractedTable]) -> f32 {
        let mut score = 0.0;
        let mut factors = 0;

        // Base ML confidence
        if layout.processing_time_ms > 0 {
            score += 0.7;
            factors += 1;
        }

        // Entity extraction confidence
        if !layout.entities.is_empty() {
            let avg_entity_conf: f32 = layout.entities.iter().map(|e| e.confidence).sum::<f32>()
                / layout.entities.len() as f32;
            score += avg_entity_conf;
            factors += 1;
        }

        // Key-value pair confidence
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

        // Table extraction confidence
        if !tables.is_empty() {
            let avg_table_conf: f32 =
                tables.iter().map(|t| t.confidence).sum::<f32>() / tables.len() as f32;
            score += avg_table_conf;
            factors += 1;
        }

        // Bonus for finding contract-specific entities
        let has_contract_number = layout
            .entities
            .iter()
            .any(|e| e.entity_type == EntityType::ContractNumber);
        if has_contract_number {
            score += 0.1;
        }

        if factors > 0 {
            (score / factors as f32).min(1.0)
        } else {
            0.3
        }
    }
}

impl Default for MlContractParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect clause type from header text
fn detect_clause_type(header: &str) -> Option<ClauseType> {
    let lower = header.to_lowercase();

    if lower.contains("payment") || lower.contains("compensation") || lower.contains("fee") {
        Some(ClauseType::PaymentTerms)
    } else if lower.contains("termination") || lower.contains("cancellation") {
        Some(ClauseType::Termination)
    } else if lower.contains("confidential")
        || lower.contains("non-disclosure")
        || lower.contains("nda")
    {
        Some(ClauseType::Confidentiality)
    } else if lower.contains("liability") || lower.contains("indemnif") {
        Some(ClauseType::Liability)
    } else if lower.contains("intellectual property")
        || lower.contains("ip rights")
        || lower.contains("ownership")
    {
        Some(ClauseType::IntellectualProperty)
    } else if lower.contains("dispute")
        || lower.contains("arbitration")
        || lower.contains("mediation")
    {
        Some(ClauseType::DisputeResolution)
    } else if lower.contains("force majeure") {
        Some(ClauseType::ForceMajeure)
    } else if lower.contains("governing law")
        || lower.contains("jurisdiction")
        || lower.contains("applicable law")
    {
        Some(ClauseType::GoverningLaw)
    } else {
        None
    }
}

/// Parse various date formats
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

    // Try "Month Day, Year" format
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_clause_type() {
        assert_eq!(
            detect_clause_type("Payment Terms"),
            Some(ClauseType::PaymentTerms)
        );
        assert_eq!(
            detect_clause_type("TERMINATION"),
            Some(ClauseType::Termination)
        );
        assert_eq!(
            detect_clause_type("Confidentiality and Non-Disclosure"),
            Some(ClauseType::Confidentiality)
        );
        assert_eq!(
            detect_clause_type("Limitation of Liability"),
            Some(ClauseType::Liability)
        );
        assert_eq!(
            detect_clause_type("Force Majeure"),
            Some(ClauseType::ForceMajeure)
        );
        assert_eq!(detect_clause_type("General Provisions"), None);
    }

    #[test]
    fn test_parse_date_formats() {
        assert!(parse_date("01/15/2024").is_some());
        assert!(parse_date("2024-01-15").is_some());
        assert!(parse_date("January 15, 2024").is_some());
    }

    #[test]
    fn test_parse_amount() {
        assert_eq!(parse_amount("$1,234.56"), Some(Decimal::new(123456, 2)));
        assert_eq!(parse_amount("€100.00"), Some(Decimal::new(10000, 2)));
        assert_eq!(parse_amount("(500.00)"), Some(Decimal::new(-50000, 2)));
    }
}
