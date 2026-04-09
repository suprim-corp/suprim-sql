//! View and sequence diff logic.

use std::collections::HashMap;

use suprim_sql::db::schema::{SequenceNode, ViewNode};

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffKind};

use super::diff_tables::diff_columns;

// ── Views ───────────────────────────────────────────────────────────────────

pub(super) fn diff_views(
    kind_label: &str,
    source: &[ViewNode],
    target: &[ViewNode],
    out: &mut Vec<DiffEntry>,
) {
    let src_map: HashMap<&str, &ViewNode> = source.iter().map(|v| (v.name.as_str(), v)).collect();
    let tgt_map: HashMap<&str, &ViewNode> = target.iter().map(|v| (v.name.as_str(), v)).collect();

    for name in sorted_keys(&src_map) {
        if !tgt_map.contains_key(name) {
            out.push(DiffEntry {
                label: format!("{kind_label}: {name}"),
                kind: DiffKind::Added,
                checked: true,
                depth: 0,
            });
        }
    }

    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            out.push(DiffEntry {
                label: format!("{kind_label}: {name}"),
                kind: DiffKind::Removed,
                checked: true,
                depth: 0,
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
                    label: format!("{kind_label}: {name}"),
                    kind: DiffKind::Modified,
                    checked: true,
                    depth: 0,
                });
                out.extend(children);
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
                label: format!("Sequence: {name}"),
                kind: DiffKind::Added,
                checked: true,
                depth: 0,
            });
        }
    }

    for name in sorted_keys(&tgt_map) {
        if !src_map.contains_key(name) {
            out.push(DiffEntry {
                label: format!("Sequence: {name}"),
                kind: DiffKind::Removed,
                checked: true,
                depth: 0,
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
                    label: format!("Sequence: {name} (modified)"),
                    kind: DiffKind::Modified,
                    checked: true,
                    depth: 0,
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
