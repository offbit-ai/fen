use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::ids::{ContractId, DocumentId};
use super::invoice::Currency;
use super::party::Party;
use crate::validation::ValidationStatus;

/// Contract types
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ContractType {
    #[default]
    ServiceAgreement,
    PurchaseOrder,
    MasterServiceAgreement,
    StatementOfWork,
    Amendment,
    Other,
}

/// Clause types
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClauseType {
    PaymentTerms,
    Termination,
    Confidentiality,
    Liability,
    IntellectualProperty,
    DisputeResolution,
    ForceMajeure,
    GoverningLaw,
    Custom(String),
}

impl Default for ClauseType {
    fn default() -> Self {
        Self::Custom("General".to_string())
    }
}

/// Contract clause
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContractClause {
    pub clause_id: String,
    pub clause_type: ClauseType,
    pub title: Option<String>,
    pub text: String,
}

impl ContractClause {
    pub fn new(clause_type: ClauseType, text: impl Into<String>) -> Self {
        Self {
            clause_id: uuid::Uuid::new_v4().to_string(),
            clause_type,
            title: None,
            text: text.into(),
        }
    }
}

/// Contract domain model
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Contract {
    pub id: ContractId,
    pub document_id: DocumentId,

    // Identification
    pub contract_number: Option<String>,
    pub title: String,
    pub contract_type: ContractType,

    // Parties
    pub parties: Vec<Party>,

    // Dates
    pub effective_date: NaiveDate,
    pub expiration_date: Option<NaiveDate>,
    pub execution_date: Option<NaiveDate>,

    // Financial terms
    pub total_value: Option<Decimal>,
    pub currency: Option<Currency>,

    // Clauses
    pub clauses: Vec<ContractClause>,

    // Validation
    pub validation_status: ValidationStatus,

    // Extraction metadata
    pub confidence_score: f32,
    pub extracted_text: String,
}

impl Contract {
    /// Create a new contract with minimal required fields
    pub fn new(title: impl Into<String>, effective_date: NaiveDate) -> Self {
        Self {
            id: ContractId::new(),
            document_id: DocumentId::new(),
            contract_number: None,
            title: title.into(),
            contract_type: ContractType::default(),
            parties: Vec::new(),
            effective_date,
            expiration_date: None,
            execution_date: None,
            total_value: None,
            currency: None,
            clauses: Vec::new(),
            validation_status: ValidationStatus::default(),
            confidence_score: 0.0,
            extracted_text: String::new(),
        }
    }
}
