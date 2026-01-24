//! Test fixtures for creating test data
//!
//! This module provides factory functions for creating test invoices,
//! contracts, and other domain objects with various configurations.

use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use fen_core::domain::{
    party::{Address, Contact},
    ClauseType, Contract, ContractClause, ContractType, Currency, DocumentId, Invoice,
    InvoiceId, LineItem, Party, PartyId,
};
use fen_core::ValidationStatus;

/// Test invoice builder for creating invoices with various configurations
#[derive(Clone)]
pub struct InvoiceFixture {
    pub invoice_number: String,
    pub invoice_date: NaiveDate,
    pub due_date: NaiveDate,
    pub vendor: Party,
    pub bill_to: Party,
    pub line_items: Vec<LineItem>,
    pub subtotal: Decimal,
    pub tax_amount: Decimal,
    pub discount_amount: Decimal,
    pub total_amount: Decimal,
    pub currency: Currency,
    pub confidence_score: f32,
    pub validation_status: ValidationStatus,
}

impl Default for InvoiceFixture {
    fn default() -> Self {
        let today = Utc::now().date_naive();
        Self {
            invoice_number: "INV-TEST-001".to_string(),
            invoice_date: today,
            due_date: today + chrono::Duration::days(30),
            vendor: Party::new("Test Vendor Inc."),
            bill_to: Party::new("Test Customer LLC"),
            line_items: vec![
                LineItem::new(1, "Widget A", dec!(10), dec!(25.00)),
                LineItem::new(2, "Service B", dec!(5), dec!(100.00)),
            ],
            subtotal: dec!(750.00),    // 10*25 + 5*100
            tax_amount: dec!(75.00),   // 10%
            discount_amount: dec!(0),
            total_amount: dec!(825.00), // 750 + 75
            currency: Currency::USD,
            confidence_score: 0.95,
            validation_status: ValidationStatus::Pending,
        }
    }
}

impl InvoiceFixture {
    /// Create a new invoice fixture with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the invoice number
    pub fn with_number(mut self, number: impl Into<String>) -> Self {
        self.invoice_number = number.into();
        self
    }

    /// Set the invoice date
    pub fn with_date(mut self, date: NaiveDate) -> Self {
        self.invoice_date = date;
        self
    }

    /// Set the due date
    pub fn with_due_date(mut self, date: NaiveDate) -> Self {
        self.due_date = date;
        self
    }

    /// Set an old date (warm tier eligible)
    pub fn old(mut self, days_ago: i64) -> Self {
        let date = Utc::now().date_naive() - chrono::Duration::days(days_ago);
        self.invoice_date = date;
        self.due_date = date + chrono::Duration::days(30);
        self
    }

    /// Set the vendor
    pub fn with_vendor(mut self, name: impl Into<String>) -> Self {
        self.vendor = Party::new(name);
        self
    }

    /// Set a vendor with full details
    pub fn with_vendor_details(
        mut self,
        name: impl Into<String>,
        tax_id: impl Into<String>,
        address: Address,
    ) -> Self {
        let mut vendor = Party::new(name);
        vendor.tax_id = Some(tax_id.into());
        vendor.address = Some(address);
        self.vendor = vendor;
        self
    }

    /// Set line items
    pub fn with_line_items(mut self, items: Vec<LineItem>) -> Self {
        self.line_items = items;
        self
    }

    /// Add a line item
    pub fn add_line_item(mut self, item: LineItem) -> Self {
        self.line_items.push(item);
        self
    }

    /// Set amounts (subtotal, tax, discount, total)
    pub fn with_amounts(
        mut self,
        subtotal: Decimal,
        tax: Decimal,
        discount: Decimal,
        total: Decimal,
    ) -> Self {
        self.subtotal = subtotal;
        self.tax_amount = tax;
        self.discount_amount = discount;
        self.total_amount = total;
        self
    }

    /// Create an invoice with math errors (subtotal doesn't match line items)
    pub fn with_math_error(mut self) -> Self {
        // Line items sum to 750, but we set subtotal to 700
        self.subtotal = dec!(700.00);
        self.total_amount = dec!(775.00); // Also wrong
        self
    }

    /// Create an invoice with total calculation error
    pub fn with_total_error(mut self) -> Self {
        // subtotal (750) + tax (75) - discount (0) should be 825, not 850
        self.total_amount = dec!(850.00);
        self
    }

    /// Create an invoice with date inconsistency (due date before invoice date)
    pub fn with_date_error(mut self) -> Self {
        self.due_date = self.invoice_date - chrono::Duration::days(10);
        self
    }

    /// Create an invoice with negative total
    pub fn with_negative_total(mut self) -> Self {
        self.total_amount = dec!(-100.00);
        self
    }

    /// Create an invoice with unknown vendor
    pub fn with_unknown_vendor(mut self) -> Self {
        self.vendor = Party::unknown();
        self
    }

    /// Create an invoice with low confidence
    pub fn with_low_confidence(mut self) -> Self {
        self.confidence_score = 0.45;
        self
    }

    /// Set confidence score
    pub fn with_confidence(mut self, score: f32) -> Self {
        self.confidence_score = score;
        self
    }

    /// Set validation status
    pub fn with_status(mut self, status: ValidationStatus) -> Self {
        self.validation_status = status;
        self
    }

    /// Set currency
    pub fn with_currency(mut self, currency: Currency) -> Self {
        self.currency = currency;
        self
    }

    /// Build the invoice
    pub fn build(self) -> Invoice {
        let document_id = DocumentId::new();
        let invoice_id = InvoiceId::new();

        Invoice {
            id: invoice_id,
            document_id,
            invoice_number: self.invoice_number,
            invoice_date: self.invoice_date,
            due_date: Some(self.due_date),
            po_number: None,
            vendor: self.vendor,
            bill_to: self.bill_to,
            currency: self.currency,
            line_items: self.line_items,
            subtotal: self.subtotal,
            tax_amount: self.tax_amount,
            discount_amount: self.discount_amount,
            total_amount: self.total_amount,
            validation_status: self.validation_status,
            confidence_score: self.confidence_score,
            extracted_text: String::new(),
        }
    }

    /// Build multiple invoices with sequential numbers
    pub fn build_batch(self, count: usize) -> Vec<Invoice> {
        (0..count)
            .map(|i| {
                let mut fixture = self.clone();
                fixture.invoice_number = format!("{}-{:03}", self.invoice_number, i + 1);
                fixture.build()
            })
            .collect()
    }
}

/// Test contract builder
#[derive(Clone)]
pub struct ContractFixture {
    pub title: String,
    pub contract_type: ContractType,
    pub parties: Vec<Party>,
    pub clauses: Vec<ContractClause>,
    pub effective_date: NaiveDate,
    pub expiration_date: Option<NaiveDate>,
}

impl Default for ContractFixture {
    fn default() -> Self {
        let today = Utc::now().date_naive();
        Self {
            title: "Test Service Agreement".to_string(),
            contract_type: ContractType::ServiceAgreement,
            parties: vec![
                Party::new("Acme Corp"),
                Party::new("Client Inc"),
            ],
            clauses: vec![
                ContractClause::new(ClauseType::PaymentTerms, "Net 30 payment terms apply"),
                ContractClause::new(ClauseType::Custom("SLA".to_string()), "99.9% uptime guaranteed"),
            ],
            effective_date: today,
            expiration_date: Some(today + chrono::Duration::days(365)),
        }
    }
}

impl ContractFixture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn with_type(mut self, contract_type: ContractType) -> Self {
        self.contract_type = contract_type;
        self
    }

    pub fn add_party(mut self, party: Party) -> Self {
        self.parties.push(party);
        self
    }

    pub fn add_clause(mut self, clause: ContractClause) -> Self {
        self.clauses.push(clause);
        self
    }

    pub fn build(self) -> Contract {
        Contract {
            id: fen_core::domain::ContractId::new(),
            document_id: DocumentId::new(),
            contract_number: None,
            title: self.title,
            contract_type: self.contract_type,
            parties: self.parties,
            effective_date: self.effective_date,
            expiration_date: self.expiration_date,
            execution_date: None,
            total_value: None,
            currency: None,
            clauses: self.clauses,
            validation_status: ValidationStatus::Pending,
            confidence_score: 0.9,
            extracted_text: String::new(),
        }
    }
}

/// Create a test address
pub fn test_address() -> Address {
    Address {
        street: Some("123 Test Street".to_string()),
        city: Some("Test City".to_string()),
        state: Some("TS".to_string()),
        postal_code: Some("12345".to_string()),
        country: Some("Test Country".to_string()),
    }
}

/// Create a test contact
pub fn test_contact() -> Contact {
    Contact {
        name: Some("John Test".to_string()),
        email: Some("john@test.com".to_string()),
        phone: Some("+1-555-0123".to_string()),
    }
}

/// Create a test party with full details
pub fn test_party_full(name: &str) -> Party {
    Party {
        id: PartyId::new(),
        name: name.to_string(),
        tax_id: Some("TAX-12345".to_string()),
        address: Some(test_address()),
        contact: Some(test_contact()),
    }
}

/// Sample invoice text for parsing tests (simulates extracted PDF text)
pub fn sample_invoice_text() -> &'static str {
    r#"
INVOICE

Invoice Number: INV-2024-001
Invoice Date: 2024-01-15
Due Date: 2024-02-14

From:
Acme Supplies Inc.
123 Business Lane
Commerce City, CA 90210
Tax ID: 12-3456789

Bill To:
Widget Corp
456 Industry Ave
Tech Town, NY 10001

Description                     Qty    Unit Price    Total
------------------------------------------------------------
Premium Widgets                  10        $25.00   $250.00
Installation Service              2       $150.00   $300.00
Support Package (Annual)          1       $500.00   $500.00

                                        Subtotal:  $1,050.00
                                        Tax (8%):     $84.00
                                        Total:     $1,134.00

Payment Terms: Net 30
Thank you for your business!
"#
}

/// Sample invoice text with errors for validation testing
pub fn sample_invoice_text_with_errors() -> &'static str {
    r#"
INVOICE

Invoice Number: INV-ERR-001
Invoice Date: 2024-02-15
Due Date: 2024-01-15

From:
Unknown Vendor

Bill To:
Test Customer

Description                     Qty    Unit Price    Total
------------------------------------------------------------
Item A                           10        $10.00   $100.00
Item B                            5        $20.00   $100.00

                                        Subtotal:    $200.00
                                        Tax (10%):    $20.00
                                        Total:       $300.00
"#
}

/// Generate random embedding vector for testing vector search
pub fn random_embedding(dim: usize) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use std::time::SystemTime;

    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();

    (0..dim)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            (seed as u64 + i as u64).hash(&mut hasher);
            let h = hasher.finish();
            // Normalize to [-1, 1] range
            (h as f32 / u64::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

/// Generate a consistent embedding for testing (same input = same output)
pub fn consistent_embedding(seed: &str, dim: usize) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    (0..dim)
        .map(|i| {
            let mut hasher = DefaultHasher::new();
            seed.hash(&mut hasher);
            i.hash(&mut hasher);
            let h = hasher.finish();
            (h as f32 / u64::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}
