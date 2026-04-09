//! DDL script generation from structured diff entries.
//!
//! Takes the source `SchemaNode` + checked `DiffEntry` list and produces
//! PostgreSQL DDL statements to bring the target schema in sync with source.

use suprim_sql::db::schema::{ColumnNode, ForeignKeyNode, IndexNode, SchemaNode, TableNode};

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffGroup, DiffKind, ObjectType};

/// Generate DDL script from checked diff groups.
pub(crate) fn generate_ddl(
    source: &SchemaNode,
    target: &SchemaNode,
    target_schema: &str,
    groups: &[DiffGroup],
) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(
        "-- Structure synchronization: {} -> {}",
        source.name, target_schema
    ));
    lines.push(format!(
        "-- Generated at: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    ));
    lines.push(String::new());

    let src_tables = table_map(&source.tables);
    let tgt_tables = table_map(&target.tables);

    for group in groups {
        for entry in &group.entries {
            if !entry.checked {
                continue;
            }
            generate_entry_ddl(
                target_schema,
                entry,
                &src_tables,
                &tgt_tables,
                source,
                &mut lines,
            );
        }
    }

    lines.join("\n")
}

fn generate_entry_ddl(
    schema: &str,
    entry: &DiffEntry,
    src_tables: &std::collections::HashMap<&str, &TableNode>,
    tgt_tables: &std::collections::HashMap<&str, &TableNode>,
    source: &SchemaNode,
    lines: &mut Vec<String>,
) {
    let name = &entry.name;
    match (entry.object_type, entry.kind) {
        // ── Tables ──────────────────────────────────────────────────────
        (ObjectType::Table, DiffKind::Added) => {
            if let Some(tbl) = src_tables.get(name.as_str()) {
                lines.push(create_table_ddl(schema, tbl));
                lines.push(String::new());
            }
        }
        (ObjectType::Table, DiffKind::Removed) => {
            lines.push(format!(
                "DROP TABLE IF EXISTS \"{schema}\".\"{name}\" CASCADE;"
            ));
            lines.push(String::new());
        }
        (ObjectType::Table, DiffKind::Modified) => {
            // Process children (columns only — indexes/FKs are top-level now)
            for child in &entry.children {
                if !child.checked {
                    continue;
                }
                if let Some(ddl) =
                    alter_table_child_ddl(schema, name, child, src_tables, tgt_tables)
                {
                    lines.push(ddl);
                }
            }
            lines.push(String::new());
        }

        // ── Indexes (top-level) ─────────────────────────────────────────
        (ObjectType::Index, DiffKind::Added) => {
            if let Some((tbl_name, idx)) = find_index_in_tables(name, src_tables) {
                lines.push(create_index_ddl(schema, tbl_name, idx));
                lines.push(String::new());
            }
        }
        (ObjectType::Index, DiffKind::Removed) => {
            lines.push(format!("DROP INDEX IF EXISTS \"{schema}\".\"{name}\";"));
            lines.push(String::new());
        }
        (ObjectType::Index, DiffKind::Modified) => {
            lines.push(format!("DROP INDEX IF EXISTS \"{schema}\".\"{name}\";"));
            if let Some((tbl_name, idx)) = find_index_in_tables(name, src_tables) {
                lines.push(create_index_ddl(schema, tbl_name, idx));
            }
            lines.push(String::new());
        }

        // ── Foreign Keys (top-level) ────────────────────────────────────
        (ObjectType::ForeignKey, DiffKind::Added) => {
            if let Some((tbl_name, fk)) = find_fk_in_tables(name, src_tables) {
                lines.push(add_foreign_key_ddl(schema, tbl_name, fk));
                lines.push(String::new());
            }
        }
        (ObjectType::ForeignKey, DiffKind::Removed) => {
            if let Some((tbl_name, _)) = find_fk_in_tables(name, tgt_tables) {
                lines.push(format!(
                    "ALTER TABLE \"{schema}\".\"{tbl_name}\" DROP CONSTRAINT \"{name}\";"
                ));
            } else {
                lines.push(format!("-- Cannot find table for FK \"{name}\" to drop"));
            }
            lines.push(String::new());
        }
        (ObjectType::ForeignKey, DiffKind::Modified) => {
            // Drop from target, re-create from source
            if let Some((tbl_name, _)) = find_fk_in_tables(name, tgt_tables) {
                lines.push(format!(
                    "ALTER TABLE \"{schema}\".\"{tbl_name}\" DROP CONSTRAINT \"{name}\";"
                ));
            }
            if let Some((tbl_name, fk)) = find_fk_in_tables(name, src_tables) {
                lines.push(add_foreign_key_ddl(schema, tbl_name, fk));
            }
            lines.push(String::new());
        }

        // ── Views ───────────────────────────────────────────────────────
        (ObjectType::View, DiffKind::Added) => {
            lines.push(format!(
                "-- TODO: CREATE VIEW \"{schema}\".\"{name}\" (definition not available from schema introspection);"
            ));
            lines.push(String::new());
        }
        (ObjectType::View, DiffKind::Removed) => {
            lines.push(format!(
                "DROP VIEW IF EXISTS \"{schema}\".\"{name}\" CASCADE;"
            ));
            lines.push(String::new());
        }
        (ObjectType::MaterializedView, DiffKind::Added) => {
            lines.push(format!(
                "-- TODO: CREATE MATERIALIZED VIEW \"{schema}\".\"{name}\" (definition not available);"
            ));
            lines.push(String::new());
        }
        (ObjectType::MaterializedView, DiffKind::Removed) => {
            lines.push(format!(
                "DROP MATERIALIZED VIEW IF EXISTS \"{schema}\".\"{name}\" CASCADE;"
            ));
            lines.push(String::new());
        }

        // ── Sequences ───────────────────────────────────────────────────
        (ObjectType::Sequence, DiffKind::Added) => {
            if let Some(seq) = source.sequences.iter().find(|s| s.name == *name) {
                lines.push(format!(
                    "CREATE SEQUENCE \"{schema}\".\"{name}\" AS {} INCREMENT BY {} MINVALUE {} MAXVALUE {} START WITH {};",
                    seq.data_type, seq.increment, seq.min_value, seq.max_value, seq.start_value
                ));
            }
            lines.push(String::new());
        }
        (ObjectType::Sequence, DiffKind::Removed) => {
            lines.push(format!(
                "DROP SEQUENCE IF EXISTS \"{schema}\".\"{name}\" CASCADE;"
            ));
            lines.push(String::new());
        }
        (ObjectType::Sequence, DiffKind::Modified) => {
            if let Some(seq) = source.sequences.iter().find(|s| s.name == *name) {
                lines.push(format!(
                    "ALTER SEQUENCE \"{schema}\".\"{name}\" AS {} INCREMENT BY {} MINVALUE {} MAXVALUE {};",
                    seq.data_type, seq.increment, seq.min_value, seq.max_value
                ));
            }
            lines.push(String::new());
        }

        _ => {}
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn table_map(tables: &[TableNode]) -> std::collections::HashMap<&str, &TableNode> {
    tables.iter().map(|t| (t.name.as_str(), t)).collect()
}

fn find_index_in_tables<'a>(
    idx_name: &str,
    tables: &'a std::collections::HashMap<&str, &TableNode>,
) -> Option<(&'a str, &'a IndexNode)> {
    for (&tbl_name, tbl) in tables {
        if let Some(idx) = tbl.indexes.iter().find(|i| i.name == idx_name) {
            return Some((tbl_name, idx));
        }
    }
    None
}

fn find_fk_in_tables<'a>(
    fk_name: &str,
    tables: &'a std::collections::HashMap<&str, &TableNode>,
) -> Option<(&'a str, &'a ForeignKeyNode)> {
    for (&tbl_name, tbl) in tables {
        if let Some(fk) = tbl.foreign_keys.iter().find(|f| f.name == fk_name) {
            return Some((tbl_name, fk));
        }
    }
    None
}

fn alter_table_child_ddl(
    schema: &str,
    table: &str,
    entry: &DiffEntry,
    src_tables: &std::collections::HashMap<&str, &TableNode>,
    _tgt_tables: &std::collections::HashMap<&str, &TableNode>,
) -> Option<String> {
    let child_name = &entry.name;
    match (entry.object_type, entry.kind) {
        (ObjectType::Column, DiffKind::Added) => {
            if let Some(tbl) = src_tables.get(table) {
                if let Some(col) = tbl.columns.iter().find(|c| c.name == *child_name) {
                    return Some(add_column_ddl(schema, table, col));
                }
            }
            Some(format!(
                "-- Cannot resolve column definition for {table}.{child_name}"
            ))
        }
        (ObjectType::Column, DiffKind::Removed) => Some(format!(
            "ALTER TABLE \"{schema}\".\"{table}\" DROP COLUMN \"{child_name}\";"
        )),
        (ObjectType::Column, DiffKind::Modified) => {
            if let Some(tbl) = src_tables.get(table) {
                if let Some(col) = tbl.columns.iter().find(|c| c.name == *child_name) {
                    return Some(alter_column_ddl(schema, table, col));
                }
            }
            None
        }
        _ => None,
    }
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
