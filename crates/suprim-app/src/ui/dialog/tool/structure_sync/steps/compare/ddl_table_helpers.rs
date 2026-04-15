//! Low-level DDL helper functions for tables, columns, indexes, and foreign keys.

use suprim_sql::db::schema::{ColumnNode, ForeignKeyNode, IndexNode, TableNode};

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffKind, ObjectType};

// ── Table map builder ──────────────────────────────────────────────────────

pub(super) fn table_map(tables: &[TableNode]) -> std::collections::HashMap<&str, &TableNode> {
    tables.iter().map(|t| (t.name.as_str(), t)).collect()
}

// ── Lookup helpers ─────────────────────────────────────────────────────────

pub(super) fn find_index_in_tables<'a>(
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

pub(super) fn find_fk_in_tables<'a>(
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

// ── ALTER TABLE child DDL ──────────────────────────────────────────────────

pub(super) fn alter_table_child_ddl(
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

// ── CREATE TABLE ───────────────────────────────────────────────────────────

pub(super) fn create_table_ddl(schema: &str, tbl: &TableNode) -> String {
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

// ── CREATE INDEX ───────────────────────────────────────────────────────────

pub(super) fn create_index_ddl(schema: &str, table: &str, idx: &IndexNode) -> String {
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

// ── ADD FOREIGN KEY ────────────────────────────────────────────────────────

pub(super) fn add_foreign_key_ddl(schema: &str, table: &str, fk: &ForeignKeyNode) -> String {
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

// ── ADD / ALTER COLUMN ─────────────────────────────────────────────────────

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
