//! DDL script generation from diff entries.
//!
//! Takes the source `SchemaNode` + checked `DiffEntry` list and produces
//! PostgreSQL DDL statements to bring the target schema in sync with source.

use suprim_sql::db::schema::{ColumnNode, ForeignKeyNode, IndexNode, SchemaNode, TableNode};

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffKind};

/// Generate DDL script from checked diff entries.
///
/// `target_schema` is the schema name on the target side (for qualified names).
pub(crate) fn generate_ddl(
    source: &SchemaNode,
    target: &SchemaNode,
    target_schema: &str,
    entries: &[DiffEntry],
) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "-- Structure synchronization: {} → {}",
        source.name, target_schema
    ));
    lines.push(format!(
        "-- Generated at: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    lines.push(String::new());

    let src_tables = table_map(&source.tables);
    let tgt_tables = table_map(&target.tables);

    let mut i = 0;
    while i < entries.len() {
        let entry = &entries[i];
        if !entry.checked {
            i += 1;
            continue;
        }

        match (&entry.kind, parse_object_type(&entry.label)) {
            // ── CREATE TABLE ────────────────────────────────────────────
            (DiffKind::Added, Some(("Table", name))) => {
                if let Some(tbl) = src_tables.get(name) {
                    lines.push(create_table_ddl(target_schema, tbl));
                    lines.push(String::new());
                    // Create indexes for the new table
                    for idx in &tbl.indexes {
                        lines.push(create_index_ddl(target_schema, name, idx));
                    }
                    // Create FKs for the new table
                    for fk in &tbl.foreign_keys {
                        lines.push(add_foreign_key_ddl(target_schema, name, fk));
                    }
                }
                // Skip child entries (columns of the new table)
                i += 1;
                while i < entries.len() && entries[i].depth > 0 {
                    i += 1;
                }
                continue;
            }

            // ── DROP TABLE ──────────────────────────────────────────────
            (DiffKind::Removed, Some(("Table", name))) => {
                lines.push(format!(
                    "DROP TABLE IF EXISTS \"{target_schema}\".\"{name}\" CASCADE;"
                ));
                lines.push(String::new());
            }

            // ── ALTER TABLE (modified) ──────────────────────────────────
            (DiffKind::Modified, Some(("Table", name))) => {
                // Process child entries for this table
                i += 1;
                while i < entries.len() && entries[i].depth > 0 {
                    let child = &entries[i];
                    if child.checked {
                        if let Some(ddl) = alter_table_child_ddl(
                            target_schema,
                            name,
                            child,
                            &src_tables,
                            &tgt_tables,
                        ) {
                            lines.push(ddl);
                        }
                    }
                    i += 1;
                }
                lines.push(String::new());
                continue;
            }

            // ── Views ───────────────────────────────────────────────────
            (DiffKind::Added, Some(("View", name))) => {
                lines.push(format!(
                    "-- TODO: CREATE VIEW \"{target_schema}\".\"{name}\" (definition not available from schema introspection);"
                ));
                lines.push(String::new());
            }
            (DiffKind::Removed, Some(("View", name))) => {
                lines.push(format!(
                    "DROP VIEW IF EXISTS \"{target_schema}\".\"{name}\" CASCADE;"
                ));
                lines.push(String::new());
            }
            (DiffKind::Added, Some(("Materialized View", name))) => {
                lines.push(format!(
                    "-- TODO: CREATE MATERIALIZED VIEW \"{target_schema}\".\"{name}\" (definition not available);"
                ));
                lines.push(String::new());
            }
            (DiffKind::Removed, Some(("Materialized View", name))) => {
                lines.push(format!(
                    "DROP MATERIALIZED VIEW IF EXISTS \"{target_schema}\".\"{name}\" CASCADE;"
                ));
                lines.push(String::new());
            }

            // ── Sequences ───────────────────────────────────────────────
            (DiffKind::Added, Some(("Sequence", name))) => {
                if let Some(seq) = source.sequences.iter().find(|s| s.name == name) {
                    lines.push(format!(
                        "CREATE SEQUENCE \"{target_schema}\".\"{name}\" AS {} INCREMENT BY {} MINVALUE {} MAXVALUE {} START WITH {};",
                        seq.data_type, seq.increment, seq.min_value, seq.max_value, seq.start_value
                    ));
                }
                lines.push(String::new());
            }
            (DiffKind::Removed, Some(("Sequence", name))) => {
                lines.push(format!(
                    "DROP SEQUENCE IF EXISTS \"{target_schema}\".\"{name}\" CASCADE;"
                ));
                lines.push(String::new());
            }
            (DiffKind::Modified, Some(("Sequence", name))) => {
                if let Some(seq) = source.sequences.iter().find(|s| s.name == name) {
                    lines.push(format!(
                        "ALTER SEQUENCE \"{target_schema}\".\"{name}\" AS {} INCREMENT BY {} MINVALUE {} MAXVALUE {};",
                        seq.data_type, seq.increment, seq.min_value, seq.max_value
                    ));
                }
                lines.push(String::new());
            }

            _ => {}
        }

        i += 1;
    }

    lines.join("\n")
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn table_map(tables: &[TableNode]) -> std::collections::HashMap<&str, &TableNode> {
    tables.iter().map(|t| (t.name.as_str(), t)).collect()
}

/// Parse "Table: users" → Some(("Table", "users"))
fn parse_object_type(label: &str) -> Option<(&str, &str)> {
    let label = label.trim();
    let colon = label.find(':')?;
    let kind = &label[..colon];
    let mut name = label[colon + 1..].trim();
    // Strip trailing modifiers like " (modified)" or " — type: ..."
    if let Some(paren) = name.find(" (") {
        name = &name[..paren];
    }
    if let Some(dash) = name.find(" —") {
        name = &name[..dash];
    }
    Some((kind, name))
}

fn create_table_ddl(schema: &str, tbl: &TableNode) -> String {
    let mut sql = format!("CREATE TABLE \"{schema}\".\"{}\" (\n", tbl.name);
    let pk_cols: Vec<&str> = tbl
        .columns
        .iter()
        .filter(|c| c.is_primary_key)
        .map(|c| c.name.as_str())
        .collect();

    for (i, col) in tbl.columns.iter().enumerate() {
        let comma = if i + 1 < tbl.columns.len() || !pk_cols.is_empty() {
            ","
        } else {
            ""
        };
        let null = if col.nullable { "" } else { " NOT NULL" };
        let default = col
            .default_value
            .as_ref()
            .map(|d| format!(" DEFAULT {d}"))
            .unwrap_or_default();
        sql.push_str(&format!(
            "    \"{}\" {}{null}{default}{comma}\n",
            col.name, col.db_type
        ));
    }

    if !pk_cols.is_empty() {
        let cols = pk_cols
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!("    PRIMARY KEY ({cols})\n"));
    }

    sql.push_str(");");
    sql
}

fn create_index_ddl(schema: &str, table: &str, idx: &IndexNode) -> String {
    let unique = if idx.is_unique { "UNIQUE " } else { "" };
    let cols = idx
        .columns
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "CREATE {unique}INDEX \"{}\" ON \"{schema}\".\"{table}\" ({cols});",
        idx.name
    )
}

fn add_foreign_key_ddl(schema: &str, table: &str, fk: &ForeignKeyNode) -> String {
    let cols = fk
        .columns
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let ref_cols = fk
        .ref_columns
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "ALTER TABLE \"{schema}\".\"{table}\" ADD CONSTRAINT \"{}\" FOREIGN KEY ({cols}) REFERENCES \"{schema}\".\"{}\" ({ref_cols});",
        fk.name, fk.ref_table
    )
}

fn alter_table_child_ddl(
    schema: &str,
    table: &str,
    entry: &DiffEntry,
    src_tables: &std::collections::HashMap<&str, &TableNode>,
    _tgt_tables: &std::collections::HashMap<&str, &TableNode>,
) -> Option<String> {
    let label = entry.label.trim();

    if let Some(rest) = label.strip_prefix("Column: ") {
        let col_name = rest.split_whitespace().next().unwrap_or(rest);
        match entry.kind {
            DiffKind::Added => {
                // Find column definition from source
                if let Some(tbl) = src_tables.get(table) {
                    if let Some(col) = tbl.columns.iter().find(|c| c.name == col_name) {
                        return Some(add_column_ddl(schema, table, col));
                    }
                }
                Some(format!(
                    "-- Cannot resolve column definition for {table}.{col_name}"
                ))
            }
            DiffKind::Removed => Some(format!(
                "ALTER TABLE \"{schema}\".\"{table}\" DROP COLUMN \"{col_name}\";"
            )),
            DiffKind::Modified => {
                if let Some(tbl) = src_tables.get(table) {
                    if let Some(col) = tbl.columns.iter().find(|c| c.name == col_name) {
                        return Some(alter_column_ddl(schema, table, col));
                    }
                }
                None
            }
        }
    } else if let Some(rest) = label.strip_prefix("Index: ") {
        let idx_name = rest.split_whitespace().next().unwrap_or(rest);
        match entry.kind {
            DiffKind::Added => {
                if let Some(tbl) = src_tables.get(table) {
                    if let Some(idx) = tbl.indexes.iter().find(|i| i.name == idx_name) {
                        return Some(create_index_ddl(schema, table, idx));
                    }
                }
                None
            }
            DiffKind::Removed => Some(format!("DROP INDEX IF EXISTS \"{schema}\".\"{idx_name}\";")),
            DiffKind::Modified => {
                // Drop and recreate
                let mut ddl = format!("DROP INDEX IF EXISTS \"{schema}\".\"{idx_name}\";\n");
                if let Some(tbl) = src_tables.get(table) {
                    if let Some(idx) = tbl.indexes.iter().find(|i| i.name == idx_name) {
                        ddl.push_str(&create_index_ddl(schema, table, idx));
                    }
                }
                Some(ddl)
            }
        }
    } else if let Some(rest) = label.strip_prefix("FK: ") {
        let fk_name = rest.split_whitespace().next().unwrap_or(rest);
        match entry.kind {
            DiffKind::Added => {
                if let Some(tbl) = src_tables.get(table) {
                    if let Some(fk) = tbl.foreign_keys.iter().find(|f| f.name == fk_name) {
                        return Some(add_foreign_key_ddl(schema, table, fk));
                    }
                }
                None
            }
            DiffKind::Removed => Some(format!(
                "ALTER TABLE \"{schema}\".\"{table}\" DROP CONSTRAINT \"{fk_name}\";"
            )),
            DiffKind::Modified => {
                let mut ddl = format!(
                    "ALTER TABLE \"{schema}\".\"{table}\" DROP CONSTRAINT \"{fk_name}\";\n"
                );
                if let Some(tbl) = src_tables.get(table) {
                    if let Some(fk) = tbl.foreign_keys.iter().find(|f| f.name == fk_name) {
                        ddl.push_str(&add_foreign_key_ddl(schema, table, fk));
                    }
                }
                Some(ddl)
            }
        }
    } else {
        None
    }
}

fn add_column_ddl(schema: &str, table: &str, col: &ColumnNode) -> String {
    let null = if col.nullable { "" } else { " NOT NULL" };
    let default = col
        .default_value
        .as_ref()
        .map(|d| format!(" DEFAULT {d}"))
        .unwrap_or_default();
    format!(
        "ALTER TABLE \"{schema}\".\"{table}\" ADD COLUMN \"{}\" {}{null}{default};",
        col.name, col.db_type
    )
}

fn alter_column_ddl(schema: &str, table: &str, col: &ColumnNode) -> String {
    let mut stmts = Vec::new();
    stmts.push(format!(
        "ALTER TABLE \"{schema}\".\"{table}\" ALTER COLUMN \"{}\" TYPE {};",
        col.name, col.db_type
    ));
    if col.nullable {
        stmts.push(format!(
            "ALTER TABLE \"{schema}\".\"{table}\" ALTER COLUMN \"{}\" DROP NOT NULL;",
            col.name
        ));
    } else {
        stmts.push(format!(
            "ALTER TABLE \"{schema}\".\"{table}\" ALTER COLUMN \"{}\" SET NOT NULL;",
            col.name
        ));
    }
    if let Some(def) = &col.default_value {
        stmts.push(format!(
            "ALTER TABLE \"{schema}\".\"{table}\" ALTER COLUMN \"{}\" SET DEFAULT {def};",
            col.name
        ));
    } else {
        stmts.push(format!(
            "ALTER TABLE \"{schema}\".\"{table}\" ALTER COLUMN \"{}\" DROP DEFAULT;",
            col.name
        ));
    }
    stmts.join("\n")
}
