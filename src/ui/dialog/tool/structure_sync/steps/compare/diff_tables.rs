//! Table-level diff: tables, columns within tables, indexes, foreign keys.

use std::collections::HashMap;

use suprim_sql::db::schema::{ColumnNode, ForeignKeyNode, IndexNode, TableNode};

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffKind};

// ── Tables ──────────────────────────────────────────────────────────────────

pub(super) fn diff_tables(source: &[TableNode], target: &[TableNode], out: &mut Vec<DiffEntry>) {
    let src_map: HashMap<&str, &TableNode> = source.iter().map(|t| (t.name.as_str(), t)).collect();
    let tgt_map: HashMap<&str, &TableNode> = target.iter().map(|t| (t.name.as_str(), t)).collect();

    // Tables only in source → need to be ADDED to target
    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            out.push(DiffEntry {
                label: format!("Table: {name}"),
                kind: DiffKind::Added,
                checked: true,
                depth: 0,
            });
            if let Some(tbl) = src_map.get(name) {
                for col in &tbl.columns {
                    out.push(DiffEntry {
                        label: format!("  Column: {} {}", col.name, col_signature(col)),
                        kind: DiffKind::Added,
                        checked: true,
                        depth: 1,
                    });
                }
            }
        }
    }

    // Tables only in target → need to be REMOVED from target
    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            out.push(DiffEntry {
                label: format!("Table: {name}"),
                kind: DiffKind::Removed,
                checked: true,
                depth: 0,
            });
        }
    }

    // Tables in both → compare structure
    for name in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(name), tgt_map.get(name)) {
            diff_single_table(name, src, tgt, out);
        }
    }
}

fn diff_single_table(name: &str, source: &TableNode, target: &TableNode, out: &mut Vec<DiffEntry>) {
    let mut children = Vec::new();

    diff_columns(&source.columns, &target.columns, &mut children);
    diff_indexes(&source.indexes, &target.indexes, &mut children);
    diff_foreign_keys(&source.foreign_keys, &target.foreign_keys, &mut children);

    if !children.is_empty() {
        out.push(DiffEntry {
            label: format!("Table: {name}"),
            kind: DiffKind::Modified,
            checked: true,
            depth: 0,
        });
        out.extend(children);
    }
}

// ── Columns (pub(crate) — also used by views diff) ─────────────────────────

pub(crate) fn diff_columns(source: &[ColumnNode], target: &[ColumnNode], out: &mut Vec<DiffEntry>) {
    let src_map: HashMap<&str, &ColumnNode> = source.iter().map(|c| (c.name.as_str(), c)).collect();
    let tgt_map: HashMap<&str, &ColumnNode> = target.iter().map(|c| (c.name.as_str(), c)).collect();

    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            let col = src_map[name];
            out.push(DiffEntry {
                label: format!("  Column: {name} {}", col_signature(col)),
                kind: DiffKind::Added,
                checked: true,
                depth: 1,
            });
        }
    }

    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            out.push(DiffEntry {
                label: format!("  Column: {name}"),
                kind: DiffKind::Removed,
                checked: true,
                depth: 1,
            });
        }
    }

    for name in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(name), tgt_map.get(name)) {
            let changes = column_changes(src, tgt);
            if !changes.is_empty() {
                out.push(DiffEntry {
                    label: format!("  Column: {name} — {changes}"),
                    kind: DiffKind::Modified,
                    checked: true,
                    depth: 1,
                });
            }
        }
    }
}

fn col_signature(col: &ColumnNode) -> String {
    let mut sig = col.db_type.clone();
    if !col.nullable {
        sig.push_str(" NOT NULL");
    }
    if col.is_primary_key {
        sig.push_str(" PK");
    }
    if let Some(def) = &col.default_value {
        sig.push_str(&format!(" DEFAULT {def}"));
    }
    sig
}

fn column_changes(src: &ColumnNode, tgt: &ColumnNode) -> String {
    let mut diffs = Vec::new();
    if src.db_type != tgt.db_type {
        diffs.push(format!("type: {} → {}", tgt.db_type, src.db_type));
    }
    if src.nullable != tgt.nullable {
        diffs.push(format!("nullable: {} → {}", tgt.nullable, src.nullable));
    }
    if src.default_value != tgt.default_value {
        let from = tgt.default_value.as_deref().unwrap_or("(none)");
        let to = src.default_value.as_deref().unwrap_or("(none)");
        diffs.push(format!("default: {from} → {to}"));
    }
    diffs.join(", ")
}

// ── Indexes ─────────────────────────────────────────────────────────────────

fn diff_indexes(source: &[IndexNode], target: &[IndexNode], out: &mut Vec<DiffEntry>) {
    let src_map: HashMap<&str, &IndexNode> = source.iter().map(|i| (i.name.as_str(), i)).collect();
    let tgt_map: HashMap<&str, &IndexNode> = target.iter().map(|i| (i.name.as_str(), i)).collect();

    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            let idx = src_map[name];
            let cols = idx.columns.join(", ");
            let uniq = if idx.is_unique { " UNIQUE" } else { "" };
            out.push(DiffEntry {
                label: format!("  Index: {name}{uniq} ({cols})"),
                kind: DiffKind::Added,
                checked: true,
                depth: 1,
            });
        }
    }

    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            out.push(DiffEntry {
                label: format!("  Index: {name}"),
                kind: DiffKind::Removed,
                checked: true,
                depth: 1,
            });
        }
    }

    for name in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(name), tgt_map.get(name)) {
            if src.columns != tgt.columns || src.is_unique != tgt.is_unique {
                out.push(DiffEntry {
                    label: format!("  Index: {name} (modified)"),
                    kind: DiffKind::Modified,
                    checked: true,
                    depth: 1,
                });
            }
        }
    }
}

// ── Foreign Keys ────────────────────────────────────────────────────────────

fn diff_foreign_keys(
    source: &[ForeignKeyNode],
    target: &[ForeignKeyNode],
    out: &mut Vec<DiffEntry>,
) {
    let src_map: HashMap<&str, &ForeignKeyNode> =
        source.iter().map(|f| (f.name.as_str(), f)).collect();
    let tgt_map: HashMap<&str, &ForeignKeyNode> =
        target.iter().map(|f| (f.name.as_str(), f)).collect();

    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            let fk = src_map[name];
            let cols = fk.columns.join(", ");
            let refs = format!("{}({})", fk.ref_table, fk.ref_columns.join(", "));
            out.push(DiffEntry {
                label: format!("  FK: {name} ({cols}) → {refs}"),
                kind: DiffKind::Added,
                checked: true,
                depth: 1,
            });
        }
    }

    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            out.push(DiffEntry {
                label: format!("  FK: {name}"),
                kind: DiffKind::Removed,
                checked: true,
                depth: 1,
            });
        }
    }

    for name in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(name), tgt_map.get(name)) {
            if src.columns != tgt.columns
                || src.ref_table != tgt.ref_table
                || src.ref_columns != tgt.ref_columns
            {
                out.push(DiffEntry {
                    label: format!("  FK: {name} (modified)"),
                    kind: DiffKind::Modified,
                    checked: true,
                    depth: 1,
                });
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn sorted_keys<'a, V>(map: &'a HashMap<&'a str, V>) -> Vec<&'a str> {
    let mut keys: Vec<&str> = map.keys().copied().collect();
    keys.sort();
    keys
}
