//! View and sequence diff logic.

use std::collections::HashMap;

use suprim_sql::db::schema::{SequenceNode, ViewNode};

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffKind, ObjectType};

use super::diff_tables::diff_columns;

// ── Views ───────────────────────────────────────────────────────────────────

pub(super) fn diff_views(
    materialized: bool,
    source: &[ViewNode],
    target: &[ViewNode],
    out: &mut Vec<DiffEntry>,
) {
    let obj_type = if materialized {
        ObjectType::MaterializedView
    } else {
        ObjectType::View
    };
    let src_map: HashMap<&str, &ViewNode> = source.iter().map(|v| (v.name.as_str(), v)).collect();
    let tgt_map: HashMap<&str, &ViewNode> = target.iter().map(|v| (v.name.as_str(), v)).collect();

    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            out.push(DiffEntry {
                object_type: obj_type,
                name: name.to_string(),
                detail: String::new(),
                kind: DiffKind::Added,
                checked: true,
                children: Vec::new(),
            });
        }
    }

    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            out.push(DiffEntry {
                object_type: obj_type,
                name: name.to_string(),
                detail: String::new(),
                kind: DiffKind::Removed,
                checked: true,
                children: Vec::new(),
            });
        }
    }

    // Views in both → compare column structure
    for name in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(name), tgt_map.get(name)) {
            let mut children = Vec::new();
            diff_columns(&src.columns, &tgt.columns, &mut children);
            if !children.is_empty() {
                out.push(DiffEntry {
                    object_type: obj_type,
                    name: name.to_string(),
                    detail: String::new(),
                    kind: DiffKind::Modified,
                    checked: true,
                    children,
                });
            }
        }
    }
}

// ── Sequences ───────────────────────────────────────────────────────────────

pub(super) fn diff_sequences(
    source: &[SequenceNode],
    target: &[SequenceNode],
    out: &mut Vec<DiffEntry>,
) {
    let src_map: HashMap<&str, &SequenceNode> =
        source.iter().map(|s| (s.name.as_str(), s)).collect();
    let tgt_map: HashMap<&str, &SequenceNode> =
        target.iter().map(|s| (s.name.as_str(), s)).collect();

    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            out.push(DiffEntry {
                object_type: ObjectType::Sequence,
                name: name.to_string(),
                detail: String::new(),
                kind: DiffKind::Added,
                checked: true,
                children: Vec::new(),
            });
        }
    }

    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            out.push(DiffEntry {
                object_type: ObjectType::Sequence,
                name: name.to_string(),
                detail: String::new(),
                kind: DiffKind::Removed,
                checked: true,
                children: Vec::new(),
            });
        }
    }

    for name in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(name), tgt_map.get(name)) {
            if src.data_type != tgt.data_type
                || src.increment != tgt.increment
                || src.min_value != tgt.min_value
                || src.max_value != tgt.max_value
            {
                out.push(DiffEntry {
                    object_type: ObjectType::Sequence,
                    name: name.to_string(),
                    detail: format!(
                        "{} inc={} → {} inc={}",
                        tgt.data_type, tgt.increment, src.data_type, src.increment
                    ),
                    kind: DiffKind::Modified,
                    checked: true,
                    children: Vec::new(),
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
