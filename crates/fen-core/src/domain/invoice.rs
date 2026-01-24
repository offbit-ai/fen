use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::cluster::TenantId;
use super::ids::{ContractId, DocumentId, InvoiceId};
use super::party::Party;
use crate::validation::ValidationStatus;

/// Currency types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Currency {
    #[default]
    USD,
    EUR,
    GBP,
    JPY,
    CAD,
    AUD,
    CHF,
    CNY,
    Other,
}

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Currency::USD => write!(f, "USD"),
            Currency::EUR => write!(f, "EUR"),
            Currency::GBP => write!(f, "GBP"),
            Currency::JPY => write!(f, "JPY"),
            Currency::CAD => write!(f, "CAD"),
            Currency::AUD => write!(f, "AUD"),
            Currency::CHF => write!(f, "CHF"),
            Currency::CNY => write!(f, "CNY"),
            Currency::Other => write!(f, "OTHER"),
        }
    }
}

/// Line item on an invoice
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LineItem {
    pub line_number: u32,
    pub description: String,
    pub quantity: Decimal,
    pub unit: Option<String>,
    pub unit_price: Decimal,
    pub tax_rate: Option<Decimal>,
    pub discount: Option<Decimal>,
    pub total: Decimal,
    pub item_code: Option<String>,
}

impl LineItem {
    /// Create a simple line item
    pub fn new(
        line_number: u32,
        description: impl Into<String>,
        quantity: Decimal,
        unit_price: Decimal,
    ) -> Self {
        let total = quantity * unit_price;
        Self {
            line_number,
            description: description.into(),
            quantity,
            unit: None,
            unit_price,
            tax_rate: None,
            discount: None,
            total,
            item_code: None,
        }
    }
}

/// Core Invoice domain model
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Invoice {
    pub id: InvoiceId,
    pub document_id: DocumentId,
    /// Tenant that owns this invoice
    pub tenant_id: TenantId,

    // Header information
    pub invoice_number: String,
    pub invoice_date: NaiveDate,
    pub due_date: Option<NaiveDate>,
    pub po_number: Option<String>,

    // Contract relationship
    /// Explicit reference to the contract this invoice is associated with.
    /// When set, ZIP queries can use this for direct matching.
    /// When None, relationship is inferred from vendor name, PO number, etc.
    pub contract_id: Option<ContractId>,
    /// Contract number reference (extracted from invoice text)
    pub contract_number: Option<String>,

    // Parties
    pub vendor: Party,
    pub bill_to: Party,

    // Financial
    pub currency: Currency,
    pub line_items: Vec<LineItem>,
    pub subtotal: Decimal,
    pub tax_amount: Decimal,
    pub discount_amount: Decimal,
    pub total_amount: Decimal,

    // Validation
    pub validation_status: ValidationStatus,

    // Extraction metadata
    pub confidence_score: f32,
    pub extracted_text: String,
}

impl Invoice {
    /// Create a new invoice with minimal required fields
    pub fn new(invoice_number: impl Into<String>, invoice_date: NaiveDate) -> Self {
        Self {
            id: InvoiceId::new(),
            document_id: DocumentId::new(),
            tenant_id: TenantId::system(),
            invoice_number: invoice_number.into(),
            invoice_date,
            due_date: None,
            po_number: None,
            contract_id: None,
            contract_number: None,
            vendor: Party::unknown(),
            bill_to: Party::unknown(),
            currency: Currency::default(),
            line_items: Vec::new(),
            subtotal: Decimal::ZERO,
            tax_amount: Decimal::ZERO,
            discount_amount: Decimal::ZERO,
            total_amount: Decimal::ZERO,
            validation_status: ValidationStatus::default(),
            confidence_score: 0.0,
            extracted_text: String::new(),
        }
    }

    /// Set the tenant for this invoice
    pub fn with_tenant(mut self, tenant_id: TenantId) -> Self {
        self.tenant_id = tenant_id;
        self
    }

    /// Link this invoice to a contract
    pub fn with_contract(mut self, contract_id: ContractId) -> Self {
        self.contract_id = Some(contract_id);
        self
    }

    /// Set contract number reference (extracted from invoice)
    pub fn with_contract_number(mut self, contract_number: impl Into<String>) -> Self {
        self.contract_number = Some(contract_number.into());
        self
    }

    /// Calculate expected total from components
    pub fn calculated_total(&self) -> Decimal {
        self.subtotal + self.tax_amount - self.discount_amount
    }

    /// Calculate sum of line items
    pub fn line_items_sum(&self) -> Decimal {
        self.line_items.iter().map(|li| li.total).sum()
    }
}
