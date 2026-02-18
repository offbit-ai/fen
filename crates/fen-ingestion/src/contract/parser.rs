use chrono::NaiveDate;
use once_cell::sync::Lazy;
use regex::Regex;
use rust_decimal::Decimal;

use fen_core::domain::{
    ClauseType, Contract, ContractClause, ContractId, ContractType, Currency, DocumentId, Party,
    TenantId,
};
use fen_core::ValidationStatus;

use crate::error::IngestionError;

// --- Pre-compiled regex patterns ---

static CONTRACT_NUMBER_PATTERNS: Lazy<[Regex; 4]> = Lazy::new(|| {
    [
        Regex::new(r"(?i)contract\s*(?:#|no\.?|number)\s*:?\s*([A-Z0-9][-A-Z0-9/]+)").unwrap(),
        Regex::new(r"(?i)agreement\s*(?:#|no\.?|number)\s*:?\s*([A-Z0-9][-A-Z0-9/]+)").unwrap(),
        Regex::new(r"(?i)(?:po|purchase\s+order)\s*(?:#|no\.?)\s*:?\s*([A-Z0-9][-A-Z0-9/]+)")
            .unwrap(),
        Regex::new(r"(?i)ref(?:erence)?\s*(?:#|no\.?)\s*:?\s*([A-Z0-9][-A-Z0-9/]+)").unwrap(),
    ]
});

static CONTRACT_TYPE_PATTERNS: Lazy<Vec<(Regex, ContractType)>> = Lazy::new(|| {
    vec![
        (
            Regex::new(r"(?i)master\s+service\s+agreement").unwrap(),
            ContractType::MasterServiceAgreement,
        ),
        (
            Regex::new(r"(?i)statement\s+of\s+work").unwrap(),
            ContractType::StatementOfWork,
        ),
        (
            Regex::new(r"(?i)purchase\s+order").unwrap(),
            ContractType::PurchaseOrder,
        ),
        (
            Regex::new(r"(?i)amendment").unwrap(),
            ContractType::Amendment,
        ),
        (
            Regex::new(r"(?i)service\s+agreement").unwrap(),
            ContractType::ServiceAgreement,
        ),
        (
            Regex::new(r"(?i)(?:agreement|contract)").unwrap(),
            ContractType::ServiceAgreement,
        ),
    ]
});

static TITLE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?im)^([A-Z][A-Z\s]{5,80}(?:AGREEMENT|CONTRACT|ORDER|AMENDMENT|STATEMENT))").unwrap()
});

static PARTY_PATTERNS: Lazy<[Regex; 4]> = Lazy::new(|| {
    [
        Regex::new(r#"(?i)(?:between|by\s+and\s+between)\s+["""]?(.+?)["""]?\s+(?:\(|and)\s+["""]?(.+?)["""]?\s*(?:\(|,|\n)"#).unwrap(),
        Regex::new(r"(?i)(?:client|buyer|customer)\s*:?\s*(.+?)(?:\n|$)").unwrap(),
        Regex::new(r"(?i)(?:vendor|provider|contractor|seller|supplier)\s*:?\s*(.+?)(?:\n|$)")
            .unwrap(),
        Regex::new(r"(?i)(?:party\s*(?:a|1))\s*:?\s*(.+?)(?:\n|$)").unwrap(),
    ]
});

static DATE_PATTERNS: Lazy<[Regex; 3]> = Lazy::new(|| {
    [
        Regex::new(r"(\d{1,2}[/-]\d{1,2}[/-]\d{2,4})").unwrap(),
        Regex::new(r"(\d{4}[/-]\d{1,2}[/-]\d{1,2})").unwrap(),
        Regex::new(
            r"(?i)(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)[a-z]*\s+\d{1,2},?\s+\d{4}",
        )
        .unwrap(),
    ]
});

static EFFECTIVE_DATE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:effective\s+date|commenc(?:es?|ement)\s+date|start\s+date)\s*:?\s*(.+?)(?:\n|$)")
        .unwrap()
});

static EXPIRATION_DATE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:expir(?:ation|es?)\s+date|end\s+date|termination\s+date)\s*:?\s*(.+?)(?:\n|$)")
        .unwrap()
});

static EXECUTION_DATE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:execution\s+date|signed?\s+date|date\s+(?:of\s+)?execut(?:ion|ed))\s*:?\s*(.+?)(?:\n|$)")
        .unwrap()
});

static AMOUNT_PATTERN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\$\s*([\d,]+\.?\d*)").unwrap());

static VALUE_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:total\s+(?:contract\s+)?value|not\s+to\s+exceed|maximum\s+(?:contract\s+)?(?:value|amount))\s*:?\s*\$\s*([\d,]+\.?\d*)")
        .unwrap()
});

static CLAUSE_PATTERNS: Lazy<Vec<(ClauseType, Regex)>> = Lazy::new(|| {
    vec![
        (
            ClauseType::PaymentTerms,
            Regex::new(r"(?is)(?:payment\s+terms?|compensation)[:\s]*(.{20,500}?)(?:\n\s*\n|\n\d+\.)").unwrap(),
        ),
        (
            ClauseType::Termination,
            Regex::new(r"(?is)(?:termination)[:\s]*(.{20,500}?)(?:\n\s*\n|\n\d+\.)").unwrap(),
        ),
        (
            ClauseType::Confidentiality,
            Regex::new(r"(?is)(?:confidential(?:ity)?|non-disclosure)[:\s]*(.{20,500}?)(?:\n\s*\n|\n\d+\.)").unwrap(),
        ),
        (
            ClauseType::Liability,
            Regex::new(r"(?is)(?:limitation\s+of\s+liability|liability)[:\s]*(.{20,500}?)(?:\n\s*\n|\n\d+\.)").unwrap(),
        ),
        (
            ClauseType::IntellectualProperty,
            Regex::new(r"(?is)(?:intellectual\s+property|ip\s+rights)[:\s]*(.{20,500}?)(?:\n\s*\n|\n\d+\.)").unwrap(),
        ),
        (
            ClauseType::DisputeResolution,
            Regex::new(r"(?is)(?:dispute\s+resolution|arbitration)[:\s]*(.{20,500}?)(?:\n\s*\n|\n\d+\.)").unwrap(),
        ),
        (
            ClauseType::ForceMajeure,
            Regex::new(r"(?is)(?:force\s+majeure)[:\s]*(.{20,500}?)(?:\n\s*\n|\n\d+\.)").unwrap(),
        ),
        (
            ClauseType::GoverningLaw,
            Regex::new(r"(?is)(?:governing\s+law|applicable\s+law|jurisdiction)[:\s]*(.{20,500}?)(?:\n\s*\n|\n\d+\.)").unwrap(),
        ),
    ]
});

/// Regex-based contract parser
pub struct ContractParser;

impl ContractParser {
    pub fn new() -> Self {
        Self
    }

    /// Parse contract from extracted text
    pub fn parse(
        &self,
        text: &str,
        document_id: DocumentId,
    ) -> Result<Contract, IngestionError> {
        let title = self.extract_title(text);
        let contract_type = self.detect_contract_type(text);
        let effective_date = self
            .extract_effective_date(text)
            .or_else(|| self.extract_first_date(text))
            .unwrap_or_else(|| chrono::Local::now().date_naive());

        Ok(Contract {
            id: ContractId::new(),
            document_id,
            tenant_id: TenantId::system(),
            contract_number: self.extract_contract_number(text),
            title,
            contract_type,
            parties: self.extract_parties(text),
            effective_date,
            expiration_date: self.extract_expiration_date(text),
            execution_date: self.extract_execution_date(text),
            total_value: self.extract_total_value(text),
            currency: Some(Currency::USD),
            clauses: self.extract_clauses(text),
            validation_status: ValidationStatus::Pending,
            confidence_score: 0.4,
            extracted_text: text.to_string(),
        })
    }

    fn extract_contract_number(&self, text: &str) -> Option<String> {
        for pattern in CONTRACT_NUMBER_PATTERNS.iter() {
            if let Some(caps) = pattern.captures(text) {
                if let Some(m) = caps.get(1) {
                    return Some(m.as_str().to_string());
                }
            }
        }
        None
    }

    fn extract_title(&self, text: &str) -> String {
        if let Some(caps) = TITLE_PATTERN.captures(text) {
            if let Some(m) = caps.get(1) {
                let title = m.as_str().trim();
                if !title.is_empty() {
                    return title.to_string();
                }
            }
        }
        // Fallback: use first 80 chars of first non-empty line
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.len() > 5 {
                let max = trimmed.len().min(80);
                return trimmed[..max].to_string();
            }
        }
        "Untitled Contract".to_string()
    }

    fn detect_contract_type(&self, text: &str) -> ContractType {
        for (pattern, contract_type) in CONTRACT_TYPE_PATTERNS.iter() {
            if pattern.is_match(text) {
                return contract_type.clone();
            }
        }
        ContractType::Other
    }

    fn extract_parties(&self, text: &str) -> Vec<Party> {
        let mut parties = Vec::new();

        // Try "between X and Y" pattern first
        if let Some(caps) = PARTY_PATTERNS[0].captures(text) {
            if let Some(m1) = caps.get(1) {
                parties.push(Party::new(m1.as_str().trim()));
            }
            if let Some(m2) = caps.get(2) {
                parties.push(Party::new(m2.as_str().trim()));
            }
            if !parties.is_empty() {
                return parties;
            }
        }

        // Try client/vendor patterns
        if let Some(caps) = PARTY_PATTERNS[1].captures(text) {
            if let Some(m) = caps.get(1) {
                parties.push(Party::new(m.as_str().trim()));
            }
        }
        if let Some(caps) = PARTY_PATTERNS[2].captures(text) {
            if let Some(m) = caps.get(1) {
                parties.push(Party::new(m.as_str().trim()));
            }
        }

        if parties.is_empty() {
            parties.push(Party::unknown());
        }

        parties
    }

    fn extract_effective_date(&self, text: &str) -> Option<NaiveDate> {
        if let Some(caps) = EFFECTIVE_DATE_PATTERN.captures(text) {
            if let Some(m) = caps.get(1) {
                return parse_date_from_context(m.as_str());
            }
        }
        None
    }

    fn extract_expiration_date(&self, text: &str) -> Option<NaiveDate> {
        if let Some(caps) = EXPIRATION_DATE_PATTERN.captures(text) {
            if let Some(m) = caps.get(1) {
                return parse_date_from_context(m.as_str());
            }
        }
        None
    }

    fn extract_execution_date(&self, text: &str) -> Option<NaiveDate> {
        if let Some(caps) = EXECUTION_DATE_PATTERN.captures(text) {
            if let Some(m) = caps.get(1) {
                return parse_date_from_context(m.as_str());
            }
        }
        None
    }

    fn extract_first_date(&self, text: &str) -> Option<NaiveDate> {
        for pattern in DATE_PATTERNS.iter() {
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

    fn extract_total_value(&self, text: &str) -> Option<Decimal> {
        // Try explicit value patterns first
        if let Some(caps) = VALUE_PATTERN.captures(text) {
            if let Some(m) = caps.get(1) {
                if let Some(amount) = parse_amount(m.as_str()) {
                    return Some(amount);
                }
            }
        }

        // Fallback: find the largest dollar amount
        let amounts: Vec<Decimal> = AMOUNT_PATTERN
            .captures_iter(text)
            .filter_map(|c| c.get(1))
            .filter_map(|m| parse_amount(m.as_str()))
            .collect();

        amounts.into_iter().max()
    }

    fn extract_clauses(&self, text: &str) -> Vec<ContractClause> {
        let mut clauses = Vec::new();

        for (clause_type, pattern) in CLAUSE_PATTERNS.iter() {
            if let Some(caps) = pattern.captures(text) {
                if let Some(m) = caps.get(1) {
                    let clause_text = m.as_str().trim();
                    if clause_text.len() >= 20 {
                        clauses.push(ContractClause::new(
                            clause_type.clone(),
                            clause_text,
                        ));
                    }
                }
            }
        }

        clauses
    }
}

impl Default for ContractParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a date from a context string (may contain surrounding text)
fn parse_date_from_context(context: &str) -> Option<NaiveDate> {
    // Try direct parsing first
    if let Some(date) = parse_date(context.trim()) {
        return Some(date);
    }

    // Try to find a date within the context string
    for pattern in DATE_PATTERNS.iter() {
        if let Some(caps) = pattern.captures(context) {
            if let Some(m) = caps.get(0) {
                if let Some(date) = parse_date(m.as_str()) {
                    return Some(date);
                }
            }
        }
    }

    None
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

    // Try "Month Day, Year" format
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
    fn test_detect_contract_type() {
        let parser = ContractParser::new();
        assert_eq!(
            parser.detect_contract_type("This is a Master Service Agreement"),
            ContractType::MasterServiceAgreement
        );
        assert_eq!(
            parser.detect_contract_type("Purchase Order #12345"),
            ContractType::PurchaseOrder
        );
        assert_eq!(
            parser.detect_contract_type("Statement of Work for Project X"),
            ContractType::StatementOfWork
        );
    }

    #[test]
    fn test_extract_contract_number() {
        let parser = ContractParser::new();
        assert_eq!(
            parser.extract_contract_number("Contract #: MSA-2024-001"),
            Some("MSA-2024-001".to_string())
        );
        assert_eq!(
            parser.extract_contract_number("Agreement No. AGR-123"),
            Some("AGR-123".to_string())
        );
    }

    #[test]
    fn test_parse_contract() {
        let parser = ContractParser::new();
        let text = r#"
SERVICE AGREEMENT

Contract No.: SA-2024-0042
Effective Date: January 15, 2024
Expiration Date: December 31, 2025

Between: Acme Corporation and Widget Inc.

Total Contract Value: $150,000.00

Payment Terms: Net 30 days from invoice date. Payments shall be made monthly.

Termination: Either party may terminate this agreement with 30 days written notice.
"#;

        let document_id = DocumentId::new();
        let contract = parser.parse(text, document_id).unwrap();

        assert_eq!(
            contract.contract_number,
            Some("SA-2024-0042".to_string())
        );
        assert_eq!(contract.contract_type, ContractType::ServiceAgreement);
        assert_eq!(contract.total_value, Some(Decimal::new(15000000, 2)));
        assert!(contract.parties.len() >= 2);
        assert!(!contract.clauses.is_empty());
    }
}
