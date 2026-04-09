//! Table-level diff: tables, columns within tables, indexes, foreign keys.
//!
//! All indexes and FKs are flat top-level entries with `parent_table` set.
//! Only columns remain nested under their parent table (Modified case).

use std::collections::HashMap;

use suprim_sql::db::schema::{ColumnNode, ForeignKeyNode, IndexNode, TableNode};

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffKind, ObjectType};

// ── Tables ──────────────────────────────────────────────────────────────────

pub(super) fn diff_tables(source: &[TableNode], target: &[TableNode], out: &mut Vec<DiffEntry>) {
    let src_map: HashMap<&str, &TableNode> = source.iter().map(|t| (t.name.as_str(), t)).collect();
    let tgt_map: HashMap<&str, &TableNode> = target.iter().map(|t| (t.name.as_str(), t)).collect();

    // Tables only in source → CREATED on target
    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            let tbl = src_map[name];
            let mut col_children = Vec::new();
            for col in &tbl.columns {
                col_children.push(make_entry(
                    ObjectType::Column,
                    &col.name,
                    col_signature(col),
                    DiffKind::Added,
                    Some(name),
                ));
            }
            out.push(DiffEntry {
                object_type: ObjectType::Table,
                name: name.to_string(),
                detail: String::new(),
                kind: DiffKind::Added,
                checked: true,
                children: col_children,
                parent_table: None,
            });
            for idx in &tbl.indexes {
                out.push(make_entry(
                    ObjectType::Index,
                    &idx.name,
                    idx_detail(idx),
                    DiffKind::Added,
                    Some(name),
                ));
            }
            for fk in &tbl.foreign_keys {
                out.push(make_entry(
                    ObjectType::ForeignKey,
                    &fk.name,
                    fk_detail(fk),
                    DiffKind::Added,
                    Some(name),
                ));
            }
        }
    }

    // Tables only in target → REMOVED from target (list ALL sub-objects flat)
    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            let tbl = tgt_map[name];
            out.push(DiffEntry {
                object_type: ObjectType::Table,
                name: name.to_string(),
                detail: String::new(),
                kind: DiffKind::Removed,
                checked: true,
                children: Vec::new(),
                parent_table: None,
            });
            for idx in &tbl.indexes {
                out.push(make_entry(
                    ObjectType::Index,
                    &idx.name,
                    idx_detail(idx),
                    DiffKind::Removed,
                    Some(name),
                ));
            }
            for fk in &tbl.foreign_keys {
                out.push(make_entry(
                    ObjectType::ForeignKey,
                    &fk.name,
                    fk_detail(fk),
                    DiffKind::Removed,
                    Some(name),
                ));
            }
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
    // Columns nested under Modified table
    let mut col_children = Vec::new();
    diff_columns(
        &source.columns,
        &target.columns,
        &mut col_children,
        Some(name),
    );

    if !col_children.is_empty() {
        out.push(DiffEntry {
            object_type: ObjectType::Table,
            name: name.to_string(),
            detail: String::new(),
            kind: DiffKind::Modified,
            checked: true,
            children: col_children,
            parent_table: None,
        });
    }

    // Indexes and FKs — flat top-level with parent_table
    diff_indexes(&source.indexes, &target.indexes, out, name);
    diff_foreign_keys(&source.foreign_keys, &target.foreign_keys, out, name);
}

// ── Columns (pub(crate) — also used by views diff) ─────────────────────────

pub(crate) fn diff_columns(
    source: &[ColumnNode],
    target: &[ColumnNode],
    out: &mut Vec<DiffEntry>,
    parent: Option<&str>,
) {
    let src_map: HashMap<&str, &ColumnNode> = source.iter().map(|c| (c.name.as_str(), c)).collect();
    let tgt_map: HashMap<&str, &ColumnNode> = target.iter().map(|c| (c.name.as_str(), c)).collect();

    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            let col = src_map[name];
            out.push(make_entry(
                ObjectType::Column,
                name,
                col_signature(col),
                DiffKind::Added,
                parent,
            ));
        }
    }

    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            let col = tgt_map[name];
            out.push(make_entry(
                ObjectType::Column,
                name,
                col_signature(col),
                DiffKind::Removed,
                parent,
            ));
        }
    }

    for name in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(name), tgt_map.get(name)) {
            let changes = column_changes(src, tgt);
            if !changes.is_empty() {
                out.push(make_entry(
                    ObjectType::Column,
                    name,
                    changes,
                    DiffKind::Modified,
                    parent,
                ));
            }
        }
    }
}

// ── Indexes ─────────────────────────────────────────────────────────────────

fn diff_indexes(
    source: &[IndexNode],
    target: &[IndexNode],
    out: &mut Vec<DiffEntry>,
    table_name: &str,
) {
    let src_map: HashMap<&str, &IndexNode> = source.iter().map(|i| (i.name.as_str(), i)).collect();
    let tgt_map: HashMap<&str, &IndexNode> = target.iter().map(|i| (i.name.as_str(), i)).collect();

    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            out.push(make_entry(
                ObjectType::Index,
                name,
                idx_detail(src_map[name]),
                DiffKind::Added,
                Some(table_name),
            ));
        }
    }

    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            out.push(make_entry(
                ObjectType::Index,
                name,
                idx_detail(tgt_map[name]),
                DiffKind::Removed,
                Some(table_name),
            ));
        }
    }

    for name in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(name), tgt_map.get(name)) {
            if src.columns != tgt.columns || src.is_unique != tgt.is_unique {
                out.push(make_entry(
                    ObjectType::Index,
                    name,
                    format!(
                        "{} {} {}",
                        idx_detail(tgt),
                        egui_phosphor::regular::ARROW_RIGHT,
                        idx_detail(src)
                    ),
                    DiffKind::Modified,
                    Some(table_name),
                ));
            }
        }
    }
}

// ── Foreign Keys ────────────────────────────────────────────────────────────

fn diff_foreign_keys(
    source: &[ForeignKeyNode],
    target: &[ForeignKeyNode],
    out: &mut Vec<DiffEntry>,
    table_name: &str,
) {
    let src_map: HashMap<&str, &ForeignKeyNode> =
        source.iter().map(|f| (f.name.as_str(), f)).collect();
    let tgt_map: HashMap<&str, &ForeignKeyNode> =
        target.iter().map(|f| (f.name.as_str(), f)).collect();

    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            out.push(make_entry(
                ObjectType::ForeignKey,
                name,
                fk_detail(src_map[name]),
                DiffKind::Added,
                Some(table_name),
            ));
        }
    }

    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            out.push(make_entry(
                ObjectType::ForeignKey,
                name,
                fk_detail(tgt_map[name]),
                DiffKind::Removed,
                Some(table_name),
            ));
        }
    }

    for name in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(name), tgt_map.get(name)) {
            if src.columns != tgt.columns
                || src.ref_table != tgt.ref_table
                || src.ref_columns != tgt.ref_columns
            {
                out.push(make_entry(
                    ObjectType::ForeignKey,
                    name,
                    format!(
                        "{} {} {}",
                        fk_detail(tgt),
                        egui_phosphor::regular::ARROW_RIGHT,
                        fk_detail(src)
                    ),
                    DiffKind::Modified,
                    Some(table_name),
                ));
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn make_entry(
    object_type: ObjectType,
    name: &str,
    detail: String,
    kind: DiffKind,
    parent: Option<&str>,
) -> DiffEntry {
    DiffEntry {
        object_type,
        name: name.to_string(),
        detail,
        kind,
        checked: true,
        children: Vec::new(),
        parent_table: parent.map(|s| s.to_string()),
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
    let arrow = egui_phosphor::regular::ARROW_RIGHT;
    let mut diffs = Vec::new();
    if src.db_type != tgt.db_type {
        diffs.push(format!("type: {} {arrow} {}", tgt.db_type, src.db_type));
    }
    if src.nullable != tgt.nullable {
        diffs.push(format!(
            "nullable: {} {arrow} {}",
            tgt.nullable, src.nullable
        ));
    }
    if src.default_value != tgt.default_value {
        let from = tgt.default_value.as_deref().unwrap_or("(none)");
        let to = src.default_value.as_deref().unwrap_or("(none)");
        diffs.push(format!("default: {from} {arrow} {to}"));
    }
    diffs.join(", ")
}

pub(super) fn idx_detail(idx: &IndexNode) -> String {
    let cols = idx.columns.join(", ");
    if idx.is_unique {
        format!("UNIQUE ({cols})")
    } else {
        format!("({cols})")
    }
}

pub(super) fn fk_detail(fk: &ForeignKeyNode) -> String {
    let cols = fk.columns.join(", ");
    let refs = format!("{}({})", fk.ref_table, fk.ref_columns.join(", "));
    format!("({cols}) {} {refs}", egui_phosphor::regular::ARROW_RIGHT)
}

fn sorted_keys<'a, V>(map: &'a HashMap<&'a str, V>) -> Vec<&'a str> {
    let mut keys: Vec<&str> = map.keys().copied().collect();
    keys.sort();
    keys
}
