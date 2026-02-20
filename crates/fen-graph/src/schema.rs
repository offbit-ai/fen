use ryugraph::Connection;

use crate::error::GraphError;

/// Node table DDL statements
const NODE_TABLES: &[&str] = &[
    "CREATE NODE TABLE IF NOT EXISTS Vendor(
        id STRING,
        name STRING,
        tax_id STRING,
        country STRING,
        PRIMARY KEY(id)
    )",
    "CREATE NODE TABLE IF NOT EXISTS Invoice(
        id STRING,
        document_id STRING,
        tenant_id STRING,
        invoice_number STRING,
        invoice_date DATE,
        due_date DATE,
        total_amount DOUBLE,
        currency STRING,
        confidence DOUBLE,
        PRIMARY KEY(id)
    )",
    "CREATE NODE TABLE IF NOT EXISTS Contract(
        id STRING,
        document_id STRING,
        tenant_id STRING,
        contract_number STRING,
        title STRING,
        contract_type STRING,
        effective_date DATE,
        expiration_date DATE,
        total_value DOUBLE,
        currency STRING,
        confidence DOUBLE,
        PRIMARY KEY(id)
    )",
    "CREATE NODE TABLE IF NOT EXISTS Party(
        id STRING,
        name STRING,
        tax_id STRING,
        role STRING,
        PRIMARY KEY(id)
    )",
    "CREATE NODE TABLE IF NOT EXISTS Clause(
        id STRING,
        clause_type STRING,
        title STRING,
        text STRING,
        PRIMARY KEY(id)
    )",
];

/// Relationship table DDL statements
const REL_TABLES: &[&str] = &[
    "CREATE REL TABLE IF NOT EXISTS SUPPLIES(FROM Vendor TO Invoice, since DATE)",
    "CREATE REL TABLE IF NOT EXISTS BILLED_TO(FROM Invoice TO Party)",
    "CREATE REL TABLE IF NOT EXISTS GOVERNED_BY(FROM Invoice TO Contract, po_number STRING)",
    "CREATE REL TABLE IF NOT EXISTS PARTY_TO(FROM Party TO Contract, role STRING)",
    "CREATE REL TABLE IF NOT EXISTS HAS_CLAUSE(FROM Contract TO Clause)",
];

/// Initialize the graph schema by creating all node and relationship tables.
/// Safe to call multiple times — uses IF NOT EXISTS.
pub fn initialize_schema(conn: &Connection) -> Result<(), GraphError> {
    for ddl in NODE_TABLES {
        conn.query(ddl)
            .map_err(|e| GraphError::SchemaInit(format!("Node table DDL failed: {e}")))?;
    }

    for ddl in REL_TABLES {
        conn.query(ddl)
            .map_err(|e| GraphError::SchemaInit(format!("Rel table DDL failed: {e}")))?;
    }

    tracing::info!("Graph schema initialized ({} node tables, {} rel tables)",
        NODE_TABLES.len(), REL_TABLES.len());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ryugraph::{Database, SystemConfig};

    #[test]
    fn test_schema_initialization() {
        let db = Database::in_memory(SystemConfig::default()).unwrap();
        let conn = Connection::new(&db).unwrap();
        initialize_schema(&conn).unwrap();
    }

    #[test]
    fn test_schema_idempotent() {
        let db = Database::in_memory(SystemConfig::default()).unwrap();
        let conn = Connection::new(&db).unwrap();
        initialize_schema(&conn).unwrap();
        // Second call should succeed (IF NOT EXISTS)
        initialize_schema(&conn).unwrap();
    }
}
