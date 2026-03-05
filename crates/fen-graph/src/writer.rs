use kyu_graph::{Connection, TypedValue};

use fen_core::domain::{Contract, Invoice};

use crate::error::GraphError;

/// Resolve or create a vendor node by name. Returns the vendor node ID.
///
/// Deduplicates vendors by name — if a vendor with the same name already exists,
/// returns its ID instead of creating a duplicate.
pub fn resolve_vendor(
    conn: &Connection,
    name: &str,
    tax_id: Option<&str>,
) -> Result<String, GraphError> {
    // Try to find existing vendor by name
    let cypher = format!(
        "MATCH (v:Vendor) WHERE v.name = '{}' RETURN v.id",
        escape_cypher(name)
    );
    let result = conn
        .query(&cypher)
        .map_err(|e| GraphError::Query(e.to_string()))?;

    for row in result.iter_rows() {
        if let Some(TypedValue::String(id)) = row.first() {
            return Ok(id.to_string());
        }
    }

    // Not found — create new vendor
    let id = uuid::Uuid::new_v4().to_string();
    let tax_id_val = tax_id.unwrap_or("");
    let cypher = format!(
        "CREATE (v:Vendor {{id: '{}', name: '{}', tax_id: '{}', country: ''}})",
        escape_cypher(&id),
        escape_cypher(name),
        escape_cypher(tax_id_val),
    );
    conn.query(&cypher)
        .map_err(|e| GraphError::Write(e.to_string()))?;

    tracing::debug!(vendor_id = %id, name = %name, "Created vendor node");
    Ok(id)
}

/// Resolve or create a party node by name. Returns the party node ID.
fn resolve_party(
    conn: &Connection,
    name: &str,
    tax_id: Option<&str>,
    role: &str,
) -> Result<String, GraphError> {
    // Try to find existing party by name
    let cypher = format!(
        "MATCH (p:Party) WHERE p.name = '{}' RETURN p.id",
        escape_cypher(name)
    );
    let result = conn
        .query(&cypher)
        .map_err(|e| GraphError::Query(e.to_string()))?;

    for row in result.iter_rows() {
        if let Some(TypedValue::String(id)) = row.first() {
            return Ok(id.to_string());
        }
    }

    // Not found — create new party
    let id = uuid::Uuid::new_v4().to_string();
    let tax_id_val = tax_id.unwrap_or("");
    let cypher = format!(
        "CREATE (p:Party {{id: '{}', name: '{}', tax_id: '{}', role: '{}'}})",
        escape_cypher(&id),
        escape_cypher(name),
        escape_cypher(tax_id_val),
        escape_cypher(role),
    );
    conn.query(&cypher)
        .map_err(|e| GraphError::Write(e.to_string()))?;

    tracing::debug!(party_id = %id, name = %name, role = %role, "Created party node");
    Ok(id)
}

/// Write an invoice and all its relationships to the graph.
///
/// Creates:
/// - Invoice node
/// - Vendor node (deduplicated by name) + SUPPLIES edge
/// - Bill-to Party node (deduplicated by name) + BILLED_TO edge
/// - GOVERNED_BY edge to Contract (if contract_id is set)
pub fn write_invoice(conn: &Connection, invoice: &Invoice) -> Result<(), GraphError> {
    let invoice_id = invoice.id.to_string();
    let document_id = invoice.document_id.to_string();
    let tenant_id = invoice.tenant_id.to_string();

    // Format dates for Cypher
    let invoice_date = invoice.invoice_date.format("%Y-%m-%d").to_string();
    let due_date = invoice
        .due_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();

    let total_amount: f64 =
        rust_decimal::prelude::ToPrimitive::to_f64(&invoice.total_amount).unwrap_or(0.0);
    let currency = invoice.currency.to_string();
    let confidence = invoice.confidence_score as f64;

    // 1. Create Invoice node
    let cypher = format!(
        "CREATE (i:Invoice {{
            id: '{id}',
            document_id: '{doc_id}',
            tenant_id: '{tenant}',
            invoice_number: '{inv_num}',
            invoice_date: '{inv_date}',
            due_date: '{due}',
            total_amount: {total},
            currency: '{currency}',
            confidence: {confidence}
        }})",
        id = escape_cypher(&invoice_id),
        doc_id = escape_cypher(&document_id),
        tenant = escape_cypher(&tenant_id),
        inv_num = escape_cypher(&invoice.invoice_number),
        inv_date = invoice_date,
        due = if due_date.is_empty() {
            &invoice_date
        } else {
            &due_date
        },
        total = total_amount,
        currency = escape_cypher(&currency),
        confidence = confidence,
    );
    conn.query(&cypher)
        .map_err(|e| GraphError::Write(format!("Create invoice node: {e}")))?;

    // 2. Resolve vendor + SUPPLIES edge
    if !invoice.vendor.is_unknown() {
        let vendor_id =
            resolve_vendor(conn, &invoice.vendor.name, invoice.vendor.tax_id.as_deref())?;

        let cypher = format!(
            "MATCH (v:Vendor) WHERE v.id = '{vid}' \
             MATCH (i:Invoice) WHERE i.id = '{iid}' \
             CREATE (v)-[:SUPPLIES {{since: '{since}'}}]->(i)",
            vid = escape_cypher(&vendor_id),
            iid = escape_cypher(&invoice_id),
            since = invoice_date,
        );
        conn.query(&cypher)
            .map_err(|e| GraphError::Write(format!("Create SUPPLIES edge: {e}")))?;
    }

    // 3. Resolve bill_to party + BILLED_TO edge
    if !invoice.bill_to.is_unknown() {
        let party_id = resolve_party(
            conn,
            &invoice.bill_to.name,
            invoice.bill_to.tax_id.as_deref(),
            "bill_to",
        )?;

        let cypher = format!(
            "MATCH (i:Invoice) WHERE i.id = '{iid}' \
             MATCH (p:Party) WHERE p.id = '{pid}' \
             CREATE (i)-[:BILLED_TO]->(p)",
            iid = escape_cypher(&invoice_id),
            pid = escape_cypher(&party_id),
        );
        conn.query(&cypher)
            .map_err(|e| GraphError::Write(format!("Create BILLED_TO edge: {e}")))?;
    }

    // 4. Link to contract if contract_id is set
    if let Some(contract_id) = &invoice.contract_id {
        let cid = contract_id.to_string();
        let po = invoice.po_number.as_deref();
        link_invoice_to_contract(conn, &invoice_id, &cid, po)?;
    }

    tracing::debug!(
        invoice_id = %invoice_id,
        invoice_number = %invoice.invoice_number,
        "Wrote invoice to graph"
    );

    Ok(())
}

/// Write a contract and all its relationships to the graph.
///
/// Creates:
/// - Contract node
/// - Party nodes (deduplicated by name) + PARTY_TO edges
/// - Clause nodes + HAS_CLAUSE edges
pub fn write_contract(conn: &Connection, contract: &Contract) -> Result<(), GraphError> {
    let contract_id = contract.id.to_string();
    let document_id = contract.document_id.to_string();
    let tenant_id = contract.tenant_id.to_string();

    let effective_date = contract.effective_date.format("%Y-%m-%d").to_string();
    let expiration_date = contract
        .expiration_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| effective_date.clone());

    let total_value: f64 = contract
        .total_value
        .and_then(|v| rust_decimal::prelude::ToPrimitive::to_f64(&v))
        .unwrap_or(0.0);

    let currency = contract
        .currency
        .map(|c| c.to_string())
        .unwrap_or_else(|| "USD".to_string());

    let contract_type = format!("{:?}", contract.contract_type);
    let confidence = contract.confidence_score as f64;

    // 1. Create Contract node
    let cypher = format!(
        "CREATE (c:Contract {{
            id: '{id}',
            document_id: '{doc_id}',
            tenant_id: '{tenant}',
            contract_number: '{num}',
            title: '{title}',
            contract_type: '{ctype}',
            effective_date: '{eff}',
            expiration_date: '{exp}',
            total_value: {total},
            currency: '{currency}',
            confidence: {confidence}
        }})",
        id = escape_cypher(&contract_id),
        doc_id = escape_cypher(&document_id),
        tenant = escape_cypher(&tenant_id),
        num = escape_cypher(contract.contract_number.as_deref().unwrap_or("")),
        title = escape_cypher(&contract.title),
        ctype = escape_cypher(&contract_type),
        eff = effective_date,
        exp = expiration_date,
        total = total_value,
        currency = escape_cypher(&currency),
        confidence = confidence,
    );
    conn.query(&cypher)
        .map_err(|e| GraphError::Write(format!("Create contract node: {e}")))?;

    // 2. Create Party nodes + PARTY_TO edges
    for party in &contract.parties {
        if party.is_unknown() {
            continue;
        }

        let party_id = resolve_party(conn, &party.name, party.tax_id.as_deref(), "contract_party")?;

        let cypher = format!(
            "MATCH (p:Party) WHERE p.id = '{pid}' \
             MATCH (c:Contract) WHERE c.id = '{cid}' \
             CREATE (p)-[:PARTY_TO {{role: 'contract_party'}}]->(c)",
            pid = escape_cypher(&party_id),
            cid = escape_cypher(&contract_id),
        );
        conn.query(&cypher)
            .map_err(|e| GraphError::Write(format!("Create PARTY_TO edge: {e}")))?;
    }

    // 3. Create Clause nodes + HAS_CLAUSE edges
    for clause in &contract.clauses {
        let clause_type = format!("{:?}", clause.clause_type);
        let clause_title = clause.title.as_deref().unwrap_or("");

        // Truncate clause text to avoid overly large nodes
        let clause_text = if clause.text.len() > 2000 {
            &clause.text[..2000]
        } else {
            &clause.text
        };

        let cypher = format!(
            "CREATE (cl:Clause {{
                id: '{id}',
                clause_type: '{ctype}',
                title: '{title}',
                text: '{text}'
            }})",
            id = escape_cypher(&clause.clause_id),
            ctype = escape_cypher(&clause_type),
            title = escape_cypher(clause_title),
            text = escape_cypher(clause_text),
        );
        conn.query(&cypher)
            .map_err(|e| GraphError::Write(format!("Create clause node: {e}")))?;

        let cypher = format!(
            "MATCH (c:Contract) WHERE c.id = '{cid}' \
             MATCH (cl:Clause) WHERE cl.id = '{clid}' \
             CREATE (c)-[:HAS_CLAUSE]->(cl)",
            cid = escape_cypher(&contract_id),
            clid = escape_cypher(&clause.clause_id),
        );
        conn.query(&cypher)
            .map_err(|e| GraphError::Write(format!("Create HAS_CLAUSE edge: {e}")))?;
    }

    tracing::debug!(
        contract_id = %contract_id,
        title = %contract.title,
        parties = contract.parties.len(),
        clauses = contract.clauses.len(),
        "Wrote contract to graph"
    );

    Ok(())
}

/// Create a GOVERNED_BY edge linking an invoice to a contract.
pub fn link_invoice_to_contract(
    conn: &Connection,
    invoice_id: &str,
    contract_id: &str,
    po_number: Option<&str>,
) -> Result<(), GraphError> {
    let po = po_number.unwrap_or("");
    let cypher = format!(
        "MATCH (i:Invoice) WHERE i.id = '{iid}' \
         MATCH (c:Contract) WHERE c.id = '{cid}' \
         CREATE (i)-[:GOVERNED_BY {{po_number: '{po}'}}]->(c)",
        iid = escape_cypher(invoice_id),
        cid = escape_cypher(contract_id),
        po = escape_cypher(po),
    );
    conn.query(&cypher)
        .map_err(|e| GraphError::Write(format!("Create GOVERNED_BY edge: {e}")))?;

    tracing::debug!(
        invoice_id = %invoice_id,
        contract_id = %contract_id,
        "Linked invoice to contract"
    );

    Ok(())
}

/// Escape single quotes in strings for Cypher literals.
pub(crate) fn escape_cypher(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_cypher() {
        assert_eq!(escape_cypher("hello"), "hello");
        assert_eq!(escape_cypher("it's"), "it\\'s");
        assert_eq!(escape_cypher("back\\slash"), "back\\\\slash");
        assert_eq!(escape_cypher("O'Brien's"), "O\\'Brien\\'s");
    }
}
