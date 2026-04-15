//! Table-level diff: tables, columns, indexes, foreign keys.
//!
//! All sub-objects (columns, indexes, FKs) are nested as children of their
//! parent table entry.

use std::collections::HashMap;

use suprim_core::db::schema::{ColumnNode, ForeignKeyNode, IndexNode, TableNode};

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffKind, ObjectType};

// ── Tables ──────────────────────────────────────────────────────────────────

pub(super) fn diff_tables(source: &[TableNode], target: &[TableNode], out: &mut Vec<DiffEntry>) {
    let src_map: HashMap<&str, &TableNode> = source.iter().map(|t| (t.name.as_str(), t)).collect();
    let tgt_map: HashMap<&str, &TableNode> = target.iter().map(|t| (t.name.as_str(), t)).collect();

    // Tables only in source → CREATE on target
    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            let tbl = src_map[name];
            let mut children = Vec::new();
            for col in &tbl.columns {
                children.push(make_entry(
                    ObjectType::Column,
                    &col.name,
                    col_signature(col),
                    DiffKind::Added,
                ));
            }
            for idx in &tbl.indexes {
                children.push(make_entry(
                    ObjectType::Index,
                    &idx.name,
                    idx_detail(idx),
                    DiffKind::Added,
                ));
            }
            for fk in &tbl.foreign_keys {
                children.push(make_entry(
                    ObjectType::ForeignKey,
                    &fk.name,
                    fk_detail(fk),
                    DiffKind::Added,
                ));
            }
            out.push(DiffEntry {
                object_type: ObjectType::Table,
                name: name.to_string(),
                detail: String::new(),
                kind: DiffKind::Added,
                checked: true,
                children,
            });
        }
    }

    // Tables only in target → DROP from target
    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            let tbl = tgt_map[name];
            let mut children = Vec::new();
            for col in &tbl.columns {
                children.push(make_entry(
                    ObjectType::Column,
                    &col.name,
                    col_signature(col),
                    DiffKind::Removed,
                ));
            }
            for idx in &tbl.indexes {
                children.push(make_entry(
                    ObjectType::Index,
                    &idx.name,
                    idx_detail(idx),
                    DiffKind::Removed,
                ));
            }
            for fk in &tbl.foreign_keys {
                children.push(make_entry(
                    ObjectType::ForeignKey,
                    &fk.name,
                    fk_detail(fk),
                    DiffKind::Removed,
                ));
            }
            out.push(DiffEntry {
                object_type: ObjectType::Table,
                name: name.to_string(),
                detail: String::new(),
                kind: DiffKind::Removed,
                checked: true,
                children,
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
            object_type: ObjectType::Table,
            name: name.to_string(),
            detail: String::new(),
            kind: DiffKind::Modified,
            checked: true,
            children,
        });
    }
}

// ── Columns (pub(crate) — also used by views diff) ─────────────────────────

pub(crate) fn diff_columns(source: &[ColumnNode], target: &[ColumnNode], out: &mut Vec<DiffEntry>) {
    let src_map: HashMap<&str, &ColumnNode> = source.iter().map(|c| (c.name.as_str(), c)).collect();
    let tgt_map: HashMap<&str, &ColumnNode> = target.iter().map(|c| (c.name.as_str(), c)).collect();

    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            out.push(make_entry(
                ObjectType::Column,
                name,
                col_signature(src_map[name]),
                DiffKind::Added,
            ));
        }
    }

    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            out.push(make_entry(
                ObjectType::Column,
                name,
                col_signature(tgt_map[name]),
                DiffKind::Removed,
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
                ));
            }
        }
    }
}

// ── Indexes ─────────────────────────────────────────────────────────────────

fn diff_indexes(source: &[IndexNode], target: &[IndexNode], out: &mut Vec<DiffEntry>) {
    let src_map: HashMap<&str, &IndexNode> = source.iter().map(|i| (i.name.as_str(), i)).collect();
    let tgt_map: HashMap<&str, &IndexNode> = target.iter().map(|i| (i.name.as_str(), i)).collect();

    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            out.push(make_entry(
                ObjectType::Index,
                name,
                idx_detail(src_map[name]),
                DiffKind::Added,
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
            ));
        }
    }

    for name in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(name), tgt_map.get(name)) {
            if src.columns != tgt.columns || src.is_unique != tgt.is_unique {
                let arrow = egui_phosphor::regular::ARROW_RIGHT;
                let detail = format!("{} {arrow} {}", idx_detail(tgt), idx_detail(src));
                out.push(make_entry(
                    ObjectType::Index,
                    name,
                    detail,
                    DiffKind::Modified,
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
            ));
        }
    }

    for name in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(name), tgt_map.get(name)) {
            if src.columns != tgt.columns
                || src.ref_table != tgt.ref_table
                || src.ref_columns != tgt.ref_columns
            {
                let arrow = egui_phosphor::regular::ARROW_RIGHT;
                let detail = format!("{} {arrow} {}", fk_detail(tgt), fk_detail(src));
                out.push(make_entry(
                    ObjectType::ForeignKey,
                    name,
                    detail,
                    DiffKind::Modified,
                ));
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn make_entry(object_type: ObjectType, name: &str, detail: String, kind: DiffKind) -> DiffEntry {
    DiffEntry {
        object_type,
        name: name.to_string(),
        detail,
        kind,
        checked: true,
        children: Vec::new(),
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
