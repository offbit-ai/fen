use kyu_graph::{Connection, TypedValue};

use crate::error::GraphError;
use crate::writer::escape_cypher;

/// A mapping rule from an RDF-imported table to an existing fen schema table.
pub struct RdfMapping<'a> {
    /// The RDF-inferred node table name (e.g., "Person", "Organization").
    pub rdf_table: &'a str,
    /// The fen schema node table to link to (e.g., "Vendor", "Party").
    pub fen_table: &'a str,
    /// The property used for matching (must exist on both tables).
    pub match_property: &'a str,
}

/// Map RDF-imported nodes to existing fen graph nodes based on shared property values.
///
/// For each mapping rule, finds nodes in both the RDF-imported table and the fen
/// schema table that share the same value for `match_property`, and creates
/// `:SAME_AS` relationship edges between them.
///
/// Returns the total number of SAME_AS edges created.
pub fn map_rdf_to_fen(
    conn: &Connection,
    mappings: &[RdfMapping<'_>],
) -> Result<usize, GraphError> {
    let mut total = 0;

    for mapping in mappings {
        // First, ensure the SAME_AS rel table exists for this pair.
        // Use IF NOT EXISTS so it's safe to call repeatedly.
        let rel_ddl = format!(
            "CREATE REL TABLE IF NOT EXISTS SAME_AS(FROM {rdf} TO {fen})",
            rdf = mapping.rdf_table,
            fen = mapping.fen_table,
        );
        conn.query(&rel_ddl)
            .map_err(|e| GraphError::Write(format!("Create SAME_AS rel table: {e}")))?;

        // Find matching nodes by property value and create edges.
        let prop = escape_cypher(mapping.match_property);
        let cypher = format!(
            "MATCH (r:{rdf}) \
             MATCH (f:{fen}) \
             WHERE r.{prop} = f.{prop} \
             CREATE (r)-[:SAME_AS]->(f) \
             RETURN count(*)",
            rdf = mapping.rdf_table,
            fen = mapping.fen_table,
            prop = prop,
        );

        let result = conn
            .query(&cypher)
            .map_err(|e| GraphError::Write(format!("Map RDF to fen: {e}")))?;

        for row in result.iter_rows() {
            if let Some(TypedValue::Int64(n)) = row.first() {
                total += *n as usize;
            }
        }
    }

    tracing::debug!(total_mappings = total, "Mapped RDF nodes to fen schema");
    Ok(total)
}

/// Default mappings for common RDF vocabularies to fen schema tables.
pub fn default_mappings() -> Vec<RdfMapping<'static>> {
    vec![
        RdfMapping {
            rdf_table: "Person",
            fen_table: "Party",
            match_property: "name",
        },
        RdfMapping {
            rdf_table: "Organization",
            fen_table: "Vendor",
            match_property: "name",
        },
    ]
}
