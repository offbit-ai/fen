use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::ids::{DocumentId, InvoiceId};
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

    // Header information
    pub invoice_number: String,
    pub invoice_date: NaiveDate,
    pub due_date: Option<NaiveDate>,
    pub po_number: Option<String>,

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
            invoice_number: invoice_number.into(),
            invoice_date,
            due_date: None,
            po_number: None,
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

    /// Calculate expected total from components
    pub fn calculated_total(&self) -> Decimal {
        self.subtotal + self.tax_amount - self.discount_amount
    }

    /// Calculate sum of line items
    pub fn line_items_sum(&self) -> Decimal {
        self.line_items.iter().map(|li| li.total).sum()
    }
}
