//! DDL script generation from structured diff entries.
//!
//! Takes the source `SchemaNode` + checked `DiffEntry` list and produces
//! PostgreSQL DDL statements to bring the target schema in sync with source.

use suprim_sql::db::schema::{ExtensionInfo, SchemaNode};

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffGroup, DiffKind, ObjectType};

use super::ddl_table_helpers::{
    add_foreign_key_ddl, alter_table_child_ddl, create_index_ddl, create_table_ddl,
    find_fk_in_tables, find_index_in_tables, table_map,
};

/// Generate DDL script from checked diff groups.
pub(crate) fn generate_ddl(
    source: &SchemaNode,
    target: &SchemaNode,
    target_schema: &str,
    groups: &[DiffGroup],
    source_extensions: &[ExtensionInfo],
    _target_extensions: &[ExtensionInfo],
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
                target,
                source_extensions,
                &mut lines,
            );
        }
    }

    lines.join("\n")
}

#[allow(clippy::too_many_arguments)]
fn generate_entry_ddl(
    schema: &str,
    entry: &DiffEntry,
    src_tables: &std::collections::HashMap<&str, &suprim_sql::db::schema::TableNode>,
    tgt_tables: &std::collections::HashMap<&str, &suprim_sql::db::schema::TableNode>,
    source: &SchemaNode,
    target: &SchemaNode,
    source_extensions: &[ExtensionInfo],
    lines: &mut Vec<String>,
) {
    let name = &entry.name;
    match (entry.object_type, entry.kind) {
        // ── Tables ──────────────────────────────────────────────────────
        (ObjectType::Table, DiffKind::Added) => {
            if let Some(tbl) = src_tables.get(name.as_str()) {
                lines.push(create_table_ddl(schema, tbl));
                lines.push(String::new());
                for child in &entry.children {
                    if !child.checked {
                        continue;
                    }
                    match child.object_type {
                        ObjectType::Index => {
                            if let Some(idx) = tbl.indexes.iter().find(|i| i.name == child.name) {
                                lines.push(create_index_ddl(schema, name, idx));
                                lines.push(String::new());
                            }
                        }
                        ObjectType::ForeignKey => {
                            if let Some(fk) = tbl.foreign_keys.iter().find(|f| f.name == child.name)
                            {
                                lines.push(add_foreign_key_ddl(schema, name, fk));
                                lines.push(String::new());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        (ObjectType::Table, DiffKind::Removed) => {
            lines.push(format!(
                "DROP TABLE IF EXISTS \"{schema}\".\"{name}\" CASCADE;"
            ));
            lines.push(String::new());
        }
        (ObjectType::Table, DiffKind::Modified) => {
            for child in &entry.children {
                if !child.checked {
                    continue;
                }
                match child.object_type {
                    ObjectType::Column => {
                        if let Some(ddl) =
                            alter_table_child_ddl(schema, name, child, src_tables, tgt_tables)
                        {
                            lines.push(ddl);
                        }
                    }
                    ObjectType::Index => match child.kind {
                        DiffKind::Added => {
                            if let Some(idx) =
                                find_index_in_tables(&child.name, src_tables).map(|(_, i)| i)
                            {
                                lines.push(create_index_ddl(schema, name, idx));
                            }
                        }
                        DiffKind::Removed => {
                            lines.push(format!(
                                "DROP INDEX IF EXISTS \"{schema}\".\"{}\";\n",
                                child.name
                            ));
                        }
                        DiffKind::Modified => {
                            lines.push(format!(
                                "DROP INDEX IF EXISTS \"{schema}\".\"{}\";",
                                child.name
                            ));
                            if let Some(idx) =
                                find_index_in_tables(&child.name, src_tables).map(|(_, i)| i)
                            {
                                lines.push(create_index_ddl(schema, name, idx));
                            }
                        }
                    },
                    ObjectType::ForeignKey => match child.kind {
                        DiffKind::Added => {
                            if let Some(fk) =
                                find_fk_in_tables(&child.name, src_tables).map(|(_, f)| f)
                            {
                                lines.push(add_foreign_key_ddl(schema, name, fk));
                            }
                        }
                        DiffKind::Removed => {
                            lines.push(format!(
                                "ALTER TABLE \"{schema}\".\"{name}\" DROP CONSTRAINT \"{}\";",
                                child.name
                            ));
                        }
                        DiffKind::Modified => {
                            lines.push(format!(
                                "ALTER TABLE \"{schema}\".\"{name}\" DROP CONSTRAINT \"{}\";",
                                child.name
                            ));
                            if let Some(fk) =
                                find_fk_in_tables(&child.name, src_tables).map(|(_, f)| f)
                            {
                                lines.push(add_foreign_key_ddl(schema, name, fk));
                            }
                        }
                    },
                    _ => {}
                }
            }
            lines.push(String::new());
        }

        // ── Top-level Index/FK (fallback) ───────────────────────────────
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
            }
            lines.push(String::new());
        }
        (ObjectType::ForeignKey, DiffKind::Modified) => {
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

        // ── Functions / Procedures ──────────────────────────────────────
        (ObjectType::Function, DiffKind::Added | DiffKind::Modified) => {
            if let Some(func) = source.functions.iter().find(|f| f.signature == *name) {
                if !func.definition.is_empty() {
                    let def = func.definition.trim_end_matches(';').trim();
                    lines.push(format!("{def};"));
                } else {
                    let kind = if func.is_procedure {
                        "PROCEDURE"
                    } else {
                        "FUNCTION"
                    };
                    lines.push(format!(
                        "-- TODO: CREATE OR REPLACE {kind} \"{schema}\".\"{name}\" (source not available for {} functions);",
                        func.language
                    ));
                }
            }
            lines.push(String::new());
        }
        (ObjectType::Function, DiffKind::Removed) => {
            let is_proc = target
                .functions
                .iter()
                .find(|f| f.signature == *name)
                .is_some_and(|f| f.is_procedure);
            let kind = if is_proc { "PROCEDURE" } else { "FUNCTION" };
            lines.push(format!(
                "DROP {kind} IF EXISTS \"{schema}\".{name} CASCADE;"
            ));
            lines.push(String::new());
        }

        // ── Extensions (database-level) ─────────────────────────────────
        (ObjectType::Extension, DiffKind::Added) => {
            if let Some(ext) = source_extensions.iter().find(|e| e.name == *name) {
                lines.push(format!(
                    "CREATE EXTENSION IF NOT EXISTS \"{}\" VERSION '{}';",
                    ext.name, ext.version
                ));
            } else {
                lines.push(format!("CREATE EXTENSION IF NOT EXISTS \"{name}\";"));
            }
            lines.push(String::new());
        }
        (ObjectType::Extension, DiffKind::Removed) => {
            lines.push(format!("DROP EXTENSION IF EXISTS \"{name}\" CASCADE;"));
            lines.push(String::new());
        }
        (ObjectType::Extension, DiffKind::Modified) => {
            if let Some(ext) = source_extensions.iter().find(|e| e.name == *name) {
                lines.push(format!(
                    "ALTER EXTENSION \"{}\" UPDATE TO '{}';",
                    ext.name, ext.version
                ));
            }
            lines.push(String::new());
        }

        _ => {}
    }
}
