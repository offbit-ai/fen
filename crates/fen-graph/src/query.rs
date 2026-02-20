use ryugraph::{Connection, Value};
use uuid::Uuid;

use fen_core::domain::{ContractId, InvoiceId};

use crate::error::GraphError;

/// Find all invoices supplied by a given vendor.
pub fn invoices_for_vendor(
    conn: &Connection,
    vendor_id: &str,
) -> Result<Vec<InvoiceId>, GraphError> {
    let cypher = format!(
        "MATCH (v:Vendor {{id: '{vid}'}})-[:SUPPLIES]->(i:Invoice)
         RETURN i.id",
        vid = crate::writer::escape_cypher(vendor_id),
    );

    let result = conn
        .query(&cypher)
        .map_err(|e| GraphError::Query(e.to_string()))?;

    let mut ids = Vec::new();
    for row in result {
        if let Some(Value::String(id_str)) = row.first() {
            if let Ok(uuid) = Uuid::parse_str(id_str) {
                ids.push(InvoiceId(uuid));
            }
        }
    }

    Ok(ids)
}

/// Find contracts related to an invoice.
///
/// Checks two paths:
/// 1. Direct: Invoice -[GOVERNED_BY]-> Contract
/// 2. Via vendor: Invoice <-[SUPPLIES]- Vendor -[SUPPLIES]-> Invoice -[GOVERNED_BY]-> Contract
pub fn related_contracts(
    conn: &Connection,
    invoice_id: &str,
) -> Result<Vec<ContractId>, GraphError> {
    // Direct relationship
    let cypher = format!(
        "MATCH (i:Invoice {{id: '{iid}'}})-[:GOVERNED_BY]->(c:Contract)
         RETURN DISTINCT c.id",
        iid = crate::writer::escape_cypher(invoice_id),
    );

    let result = conn
        .query(&cypher)
        .map_err(|e| GraphError::Query(e.to_string()))?;

    let mut ids = Vec::new();
    for row in result {
        if let Some(Value::String(id_str)) = row.first() {
            if let Ok(uuid) = Uuid::parse_str(id_str) {
                ids.push(ContractId(uuid));
            }
        }
    }

    // Also check via shared vendor (2-hop)
    let cypher = format!(
        "MATCH (i:Invoice {{id: '{iid}'}})<-[:SUPPLIES]-(v:Vendor)-[:SUPPLIES]->(i2:Invoice)-[:GOVERNED_BY]->(c:Contract)
         WHERE i2.id <> '{iid}'
         RETURN DISTINCT c.id",
        iid = crate::writer::escape_cypher(invoice_id),
    );

    let result = conn
        .query(&cypher)
        .map_err(|e| GraphError::Query(e.to_string()))?;

    for row in result {
        if let Some(Value::String(id_str)) = row.first() {
            if let Ok(uuid) = Uuid::parse_str(id_str) {
                if !ids.contains(&ContractId(uuid)) {
                    ids.push(ContractId(uuid));
                }
            }
        }
    }

    Ok(ids)
}

/// Execute a raw Cypher query and return results as string vectors.
pub fn execute_cypher(
    conn: &Connection,
    cypher: &str,
) -> Result<Vec<Vec<String>>, GraphError> {
    let result = conn
        .query(cypher)
        .map_err(|e| GraphError::Query(e.to_string()))?;

    let mut rows = Vec::new();
    for row in result {
        let string_row: Vec<String> = row
            .iter()
            .map(|v| format!("{:?}", v))
            .collect();
        rows.push(string_row);
    }

    Ok(rows)
}

/// Count nodes of a given label.
pub fn count_nodes(conn: &Connection, label: &str) -> Result<u64, GraphError> {
    let cypher = format!("MATCH (n:{label}) RETURN count(n)");
    let result = conn
        .query(&cypher)
        .map_err(|e| GraphError::Query(e.to_string()))?;

    for row in result {
        if let Some(Value::Int64(n)) = row.first() {
            return Ok(*n as u64);
        }
    }

    Ok(0)
}

/// Find all vendors (returns id, name pairs).
pub fn list_vendors(conn: &Connection) -> Result<Vec<(String, String)>, GraphError> {
    let cypher = "MATCH (v:Vendor) RETURN v.id, v.name";
    let result = conn
        .query(cypher)
        .map_err(|e| GraphError::Query(e.to_string()))?;

    let mut vendors = Vec::new();
    for row in result {
        if let (Some(Value::String(id)), Some(Value::String(name))) =
            (row.first(), row.get(1))
        {
            vendors.push((id.clone(), name.clone()));
        }
    }

    Ok(vendors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use ryugraph::{Database, SystemConfig};

    fn setup_db() -> Database {
        let db = Database::in_memory(SystemConfig::default()).unwrap();
        {
            let conn = Connection::new(&db).unwrap();
            schema::initialize_schema(&conn).unwrap();
        }
        db
    }

    #[test]
    fn test_count_nodes_empty() {
        let db = setup_db();
        let conn = Connection::new(&db).unwrap();
        assert_eq!(count_nodes(&conn, "Invoice").unwrap(), 0);
        assert_eq!(count_nodes(&conn, "Vendor").unwrap(), 0);
    }

    #[test]
    fn test_list_vendors_empty() {
        let db = setup_db();
        let conn = Connection::new(&db).unwrap();
        assert!(list_vendors(&conn).unwrap().is_empty());
    }
}
