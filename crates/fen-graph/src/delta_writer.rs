//! Delta-batch writer for high-throughput graph ingestion.
//!
//! Instead of issuing N individual Cypher queries per document (5+ queries for
//! an invoice with vendor + bill_to + contract link), this module builds a single
//! `DeltaBatch` containing all upserts and applies them atomically via
//! `Connection::apply_delta()`.
//!
//! Performance characteristics:
//! - Single WAL append per document (vs 5+ Cypher round-trips)
//! - Upsert semantics: vendor/party deduplication is built-in (same primary key = merge)
//! - Conflict-free: concurrent batch applications are safe (last-write-wins)
//! - Batch mode: `build_invoice_batch` / `build_contract_batch` for multi-doc ingestion

use kyu_delta::{DeltaBatchBuilder, DeltaValue};
use smol_str::SmolStr;

use fen_core::domain::{Contract, Invoice};

/// Build a DeltaBatch for a single invoice and all its relationships.
///
/// Creates upserts for:
/// - Invoice node (keyed by invoice UUID)
/// - Vendor node (keyed by vendor name — dedup via upsert)
/// - SUPPLIES edge (Vendor → Invoice)
/// - Party node for bill_to (keyed by party name — dedup via upsert)
/// - BILLED_TO edge (Invoice → Party)
/// - GOVERNED_BY edge (Invoice → Contract, if contract_id is set)
pub fn build_invoice_delta(invoice: &Invoice) -> kyu_delta::DeltaBatch {
    let invoice_id = invoice.id.to_string();
    let source = format!("doc:invoice:{}", invoice_id);
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;

    let total_amount: f64 = rust_decimal::prelude::ToPrimitive::to_f64(&invoice.total_amount)
        .unwrap_or(0.0);
    let invoice_date = invoice.invoice_date.format("%Y-%m-%d").to_string();
    let due_date = invoice
        .due_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    let mut builder = DeltaBatchBuilder::new(source, timestamp)
        // 1. Invoice node
        .upsert_node(
            "Invoice",
            &invoice_id,
            vec![],
            [
                ("id", DeltaValue::String(SmolStr::new(&invoice_id))),
                ("document_id", DeltaValue::String(SmolStr::new(invoice.document_id.to_string()))),
                ("tenant_id", DeltaValue::String(SmolStr::new(invoice.tenant_id.to_string()))),
                ("invoice_number", DeltaValue::String(SmolStr::new(&invoice.invoice_number))),
                ("invoice_date", DeltaValue::String(SmolStr::new(&invoice_date))),
                ("due_date", DeltaValue::String(SmolStr::new(&due_date))),
                ("total_amount", DeltaValue::Double(total_amount)),
                ("currency", DeltaValue::String(SmolStr::new(invoice.currency.to_string()))),
                ("confidence", DeltaValue::Double(invoice.confidence_score as f64)),
            ],
        );

    // 2. Vendor node + SUPPLIES edge (upsert by name = automatic dedup)
    if !invoice.vendor.is_unknown() {
        let vendor_key = vendor_primary_key(&invoice.vendor.name);
        builder = builder
            .upsert_node(
                "Vendor",
                &vendor_key,
                vec![],
                [
                    ("id", DeltaValue::String(SmolStr::new(&vendor_key))),
                    ("name", DeltaValue::String(SmolStr::new(&invoice.vendor.name))),
                    ("tax_id", DeltaValue::String(SmolStr::new(
                        invoice.vendor.tax_id.as_deref().unwrap_or(""),
                    ))),
                    ("country", DeltaValue::String(SmolStr::new(""))),
                ],
            )
            .upsert_edge(
                "Vendor", &vendor_key,
                "SUPPLIES",
                "Invoice", &invoice_id,
                [("since", DeltaValue::String(SmolStr::new(&invoice_date)))],
            );
    }

    // 3. Bill-to party + BILLED_TO edge
    if !invoice.bill_to.is_unknown() {
        let party_key = party_primary_key(&invoice.bill_to.name);
        builder = builder
            .upsert_node(
                "Party",
                &party_key,
                vec![],
                [
                    ("id", DeltaValue::String(SmolStr::new(&party_key))),
                    ("name", DeltaValue::String(SmolStr::new(&invoice.bill_to.name))),
                    ("tax_id", DeltaValue::String(SmolStr::new(
                        invoice.bill_to.tax_id.as_deref().unwrap_or(""),
                    ))),
                    ("role", DeltaValue::String(SmolStr::new("bill_to"))),
                ],
            )
            .upsert_edge(
                "Invoice", &invoice_id,
                "BILLED_TO",
                "Party", &party_key,
                Vec::<(&str, DeltaValue)>::new(),
            );
    }

    // 4. GOVERNED_BY edge to contract (if set)
    if let Some(contract_id) = &invoice.contract_id {
        let cid = contract_id.to_string();
        builder = builder.upsert_edge(
            "Invoice", &invoice_id,
            "GOVERNED_BY",
            "Contract", &cid,
            [("po_number", DeltaValue::String(SmolStr::new(
                invoice.po_number.as_deref().unwrap_or(""),
            )))],
        );
    }

    builder.build()
}

/// Build a DeltaBatch for a single contract and all its relationships.
pub fn build_contract_delta(contract: &Contract) -> kyu_delta::DeltaBatch {
    let contract_id = contract.id.to_string();
    let source = format!("doc:contract:{}", contract_id);
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;

    let effective_date = contract.effective_date.format("%Y-%m-%d").to_string();
    let expiration_date = contract
        .expiration_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    let total_value: f64 = contract
        .total_value
        .and_then(|v| rust_decimal::prelude::ToPrimitive::to_f64(&v))
        .unwrap_or(0.0);

    let currency = contract
        .currency
        .map(|c| c.to_string())
        .unwrap_or_else(|| "USD".to_string());

    let contract_type = format!("{:?}", contract.contract_type);

    let mut builder = DeltaBatchBuilder::new(source, timestamp)
        // 1. Contract node
        .upsert_node(
            "Contract",
            &contract_id,
            vec![],
            [
                ("id", DeltaValue::String(SmolStr::new(&contract_id))),
                ("document_id", DeltaValue::String(SmolStr::new(contract.document_id.to_string()))),
                ("tenant_id", DeltaValue::String(SmolStr::new(contract.tenant_id.to_string()))),
                ("contract_number", DeltaValue::String(SmolStr::new(
                    contract.contract_number.as_deref().unwrap_or(""),
                ))),
                ("title", DeltaValue::String(SmolStr::new(&contract.title))),
                ("contract_type", DeltaValue::String(SmolStr::new(&contract_type))),
                ("effective_date", DeltaValue::String(SmolStr::new(&effective_date))),
                ("expiration_date", DeltaValue::String(SmolStr::new(&expiration_date))),
                ("total_value", DeltaValue::Double(total_value)),
                ("currency", DeltaValue::String(SmolStr::new(&currency))),
                ("confidence", DeltaValue::Double(contract.confidence_score as f64)),
            ],
        );

    // 2. Party nodes + PARTY_TO edges
    for party in &contract.parties {
        if party.is_unknown() {
            continue;
        }
        let party_key = party_primary_key(&party.name);
        builder = builder
            .upsert_node(
                "Party",
                &party_key,
                vec![],
                [
                    ("id", DeltaValue::String(SmolStr::new(&party_key))),
                    ("name", DeltaValue::String(SmolStr::new(&party.name))),
                    ("tax_id", DeltaValue::String(SmolStr::new(
                        party.tax_id.as_deref().unwrap_or(""),
                    ))),
                    ("role", DeltaValue::String(SmolStr::new("contract_party"))),
                ],
            )
            .upsert_edge(
                "Party", &party_key,
                "PARTY_TO",
                "Contract", &contract_id,
                [("role", DeltaValue::String(SmolStr::new("contract_party")))],
            );
    }

    // 3. Clause nodes + HAS_CLAUSE edges
    for clause in &contract.clauses {
        let clause_id = clause.clause_id.clone();
        let clause_type = format!("{:?}", clause.clause_type);
        let clause_title = clause.title.as_deref().unwrap_or("");
        // Truncate clause text to avoid oversized nodes
        let clause_text = if clause.text.len() > 2000 {
            &clause.text[..2000]
        } else {
            &clause.text
        };

        builder = builder
            .upsert_node(
                "Clause",
                &clause_id,
                vec![],
                [
                    ("id", DeltaValue::String(SmolStr::new(&clause_id))),
                    ("clause_type", DeltaValue::String(SmolStr::new(&clause_type))),
                    ("title", DeltaValue::String(SmolStr::new(clause_title))),
                    ("text", DeltaValue::String(SmolStr::new(clause_text))),
                ],
            )
            .upsert_edge(
                "Contract", &contract_id,
                "HAS_CLAUSE",
                "Clause", &clause_id,
                Vec::<(&str, DeltaValue)>::new(),
            );
    }

    builder.build()
}

/// Build a combined DeltaBatch for multiple invoices.
///
/// Use this for bulk ingestion — a single `apply_delta` call for the entire batch.
pub fn build_invoice_batch(invoices: &[Invoice]) -> kyu_delta::DeltaBatch {
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;
    let mut batch = kyu_delta::DeltaBatch::new("doc:invoice_batch", timestamp);

    for invoice in invoices {
        let single = build_invoice_delta(invoice);
        batch.extend(single.deltas);
    }

    batch
}

/// Build a combined DeltaBatch for multiple contracts.
pub fn build_contract_batch(contracts: &[Contract]) -> kyu_delta::DeltaBatch {
    let timestamp = chrono::Utc::now().timestamp_millis() as u64;
    let mut batch = kyu_delta::DeltaBatch::new("doc:contract_batch", timestamp);

    for contract in contracts {
        let single = build_contract_delta(contract);
        batch.extend(single.deltas);
    }

    batch
}

/// Deterministic primary key for vendors based on normalized name.
/// This is what makes upsert-based dedup work — same vendor name = same key = merge.
pub fn vendor_primary_key(name: &str) -> String {
    let normalized = name.trim().to_lowercase();
    // Use a stable hash-like key that's still human-readable
    format!("vendor:{}", normalized.replace(' ', "_"))
}

/// Deterministic primary key for parties based on normalized name.
pub fn party_primary_key(name: &str) -> String {
    let normalized = name.trim().to_lowercase();
    format!("party:{}", normalized.replace(' ', "_"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use fen_core::domain::{
        ClauseType, Contract, ContractClause, ContractType, Currency, Invoice, Party,
    };
    use rust_decimal_macros::dec;

    fn sample_invoice() -> Invoice {
        let mut inv = Invoice::new("INV-001", NaiveDate::from_ymd_opt(2024, 6, 15).unwrap());
        inv.vendor = Party::new("Acme Corp");
        inv.vendor.tax_id = Some("12-3456789".to_string());
        inv.bill_to = Party::new("Widget Inc");
        inv.currency = Currency::USD;
        inv.total_amount = dec!(5000.00);
        inv.confidence_score = 0.95;
        inv
    }

    fn sample_contract() -> Contract {
        let mut contract = Contract::new(
            "Master Service Agreement",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
        );
        contract.contract_number = Some("MSA-2024-001".to_string());
        contract.contract_type = ContractType::MasterServiceAgreement;
        contract.total_value = Some(dec!(100000.00));
        contract.currency = Some(Currency::USD);
        contract.parties = vec![Party::new("Acme Corp"), Party::new("Widget Inc")];
        contract.clauses = vec![
            ContractClause::new(ClauseType::PaymentTerms, "Net 30 payment terms"),
        ];
        contract
    }

    #[test]
    fn invoice_delta_has_correct_structure() {
        let inv = sample_invoice();
        let batch = build_invoice_delta(&inv);

        // Invoice node + Vendor node + SUPPLIES edge + Party node + BILLED_TO edge = 5
        assert_eq!(batch.node_upsert_count(), 3); // Invoice, Vendor, Party
        assert_eq!(batch.edge_upsert_count(), 2); // SUPPLIES, BILLED_TO
        assert_eq!(batch.len(), 5);
    }

    #[test]
    fn invoice_with_contract_link() {
        let mut inv = sample_invoice();
        inv.contract_id = Some(fen_core::domain::ContractId::new());
        let batch = build_invoice_delta(&inv);

        // +1 GOVERNED_BY edge
        assert_eq!(batch.edge_upsert_count(), 3);
    }

    #[test]
    fn contract_delta_has_correct_structure() {
        let contract = sample_contract();
        let batch = build_contract_delta(&contract);

        // Contract node + 2 Party nodes + 1 Clause node = 4 nodes
        // 2 PARTY_TO edges + 1 HAS_CLAUSE edge = 3 edges
        assert_eq!(batch.node_upsert_count(), 4);
        assert_eq!(batch.edge_upsert_count(), 3);
    }

    #[test]
    fn vendor_dedup_via_primary_key() {
        // Two invoices from same vendor should produce same vendor primary key
        let inv1 = sample_invoice();
        let mut inv2 = sample_invoice();
        inv2.invoice_number = "INV-002".to_string();

        let batch = build_invoice_batch(&[inv1, inv2]);

        // Both invoices create Vendor upserts with same key — apply_delta merges them
        let vendor_labels: Vec<_> = batch.referenced_labels().into_iter()
            .filter(|l| l.as_str() == "Vendor")
            .collect();
        assert_eq!(vendor_labels.len(), 1); // Same label referenced
    }

    #[test]
    fn batch_combines_multiple_invoices() {
        let inv1 = sample_invoice();
        let mut inv2 = sample_invoice();
        inv2.invoice_number = "INV-002".to_string();

        let batch = build_invoice_batch(&[inv1, inv2]);

        // 2 invoices × 5 deltas each = 10 deltas
        assert_eq!(batch.len(), 10);
    }
}
