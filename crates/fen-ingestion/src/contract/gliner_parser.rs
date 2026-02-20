use chrono::NaiveDate;
use rust_decimal::Decimal;

use fen_core::domain::{
    ContractId, ContractType, Currency, DocumentId, Party, TenantId,
    Contract,
};
use fen_core::ValidationStatus;
use fen_ml::gliner::types::GlinerExtractionResult;

use crate::error::IngestionError;

/// Entity labels for contract extraction
pub const CONTRACT_LABELS: &[&str] = &[
    "contract_number",
    "party_name",
    "effective_date",
    "expiration_date",
    "contract_value",
    "payment_terms",
    "clause_type",
    "contract_type",
];

/// Labels that should ideally be found for a satisfactory extraction
pub const CONTRACT_EXPECTED: &[&str] = &["party_name", "effective_date"];

/// GLiNER-backed contract parser: maps NER entities to Contract domain struct
pub struct GlinerContractParser;

impl GlinerContractParser {
    pub fn new() -> Self {
        Self
    }

    /// Build a Contract from GLiNER extraction results
    pub fn build(
        &self,
        result: &GlinerExtractionResult,
        document_id: DocumentId,
        text: &str,
        confidence: f32,
    ) -> Result<Contract, IngestionError> {
        let contract_number = result
            .best_entity("contract_number")
            .map(|e| e.text.clone());

        let contract_type = result
            .best_entity("contract_type")
            .map(|e| detect_contract_type(&e.text))
            .unwrap_or_else(|| detect_contract_type(text));

        // Collect all party_name entities
        let parties: Vec<Party> = result
            .entities_for_label("party_name")
            .into_iter()
            .map(|e| Party::new(e.text.trim()))
            .collect();

        let parties = if parties.is_empty() {
            vec![Party::unknown()]
        } else {
            parties
        };

        let effective_date = result
            .best_entity("effective_date")
            .and_then(|e| parse_date(&e.text))
            .unwrap_or_else(|| chrono::Local::now().date_naive());

        let expiration_date = result
            .best_entity("expiration_date")
            .and_then(|e| parse_date(&e.text));

        let total_value = result
            .best_entity("contract_value")
            .and_then(|e| parse_amount(&e.text));

        let currency = Some(detect_currency(text));

        // Derive title from first line or contract type
        let title = derive_title(text, &contract_type);

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
            execution_date: None,
            total_value,
            currency,
            clauses: Vec::new(), // clause extraction is separate
            validation_status: ValidationStatus::Pending,
            confidence_score: confidence,
            extracted_text: text.to_string(),
        })
    }
}

impl Default for GlinerContractParser {
    fn default() -> Self {
        Self::new()
    }
}

fn derive_title(text: &str, contract_type: &ContractType) -> String {
    // Try first non-empty line as title
    if let Some(first_line) = text.lines().find(|l| !l.trim().is_empty()) {
        let trimmed = first_line.trim();
        if trimmed.len() > 5 && trimmed.len() < 200 {
            return trimmed.to_string();
        }
    }

    format!("{:?}", contract_type)
}

fn detect_contract_type(text: &str) -> ContractType {
    let lower = text.to_lowercase();
    if lower.contains("master service agreement") || lower.contains("msa") {
        ContractType::MasterServiceAgreement
    } else if lower.contains("statement of work") || lower.contains("sow") {
        ContractType::StatementOfWork
    } else if lower.contains("purchase order") {
        ContractType::PurchaseOrder
    } else if lower.contains("amendment") {
        ContractType::Amendment
    } else if lower.contains("service agreement") || lower.contains("service contract") {
        ContractType::ServiceAgreement
    } else {
        ContractType::Other
    }
}

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
    fn test_gliner_contract_parser_full() {
        let result = GlinerExtractionResult {
            entities: vec![
                make_entity("contract_number", "SA-2024-0042", 0.92),
                make_entity("party_name", "Acme Corporation", 0.90),
                make_entity("party_name", "Widget Inc", 0.88),
                make_entity("effective_date", "January 15, 2024", 0.85),
                make_entity("expiration_date", "December 31, 2025", 0.83),
                make_entity("contract_value", "$150,000.00", 0.87),
                make_entity("contract_type", "Service Agreement", 0.80),
            ],
            tier: ModelTier::Medium,
            escalated: false,
            processing_time_ms: 70,
        };

        let parser = GlinerContractParser::new();
        let contract = parser
            .build(
                &result,
                DocumentId::new(),
                "Service Agreement\nBetween Acme Corporation and Widget Inc",
                0.88,
            )
            .unwrap();

        assert_eq!(
            contract.contract_number,
            Some("SA-2024-0042".to_string())
        );
        assert_eq!(contract.parties.len(), 2);
        assert_eq!(contract.parties[0].name, "Acme Corporation");
        assert_eq!(contract.parties[1].name, "Widget Inc");
        assert_eq!(
            contract.effective_date,
            NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()
        );
        assert_eq!(
            contract.expiration_date,
            Some(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap())
        );
        assert_eq!(contract.total_value, Some(Decimal::new(15000000, 2)));
        assert_eq!(contract.contract_type, ContractType::ServiceAgreement);
        assert_eq!(contract.confidence_score, 0.88);
    }

    #[test]
    fn test_gliner_contract_parser_minimal() {
        let result = GlinerExtractionResult {
            entities: vec![make_entity("party_name", "Acme Corp", 0.7)],
            tier: ModelTier::Large,
            escalated: true,
            processing_time_ms: 200,
        };

        let parser = GlinerContractParser::new();
        let contract = parser
            .build(&result, DocumentId::new(), "Some contract text", 0.4)
            .unwrap();

        assert!(contract.contract_number.is_none());
        assert_eq!(contract.parties.len(), 1);
        assert_eq!(contract.parties[0].name, "Acme Corp");
        assert!(contract.total_value.is_none());
    }

    #[test]
    fn test_detect_contract_type() {
        assert_eq!(
            detect_contract_type("Master Service Agreement"),
            ContractType::MasterServiceAgreement
        );
        assert_eq!(
            detect_contract_type("Statement of Work #5"),
            ContractType::StatementOfWork
        );
        assert_eq!(
            detect_contract_type("Purchase Order PO-123"),
            ContractType::PurchaseOrder
        );
        assert_eq!(
            detect_contract_type("Some random text"),
            ContractType::Other
        );
    }
}
