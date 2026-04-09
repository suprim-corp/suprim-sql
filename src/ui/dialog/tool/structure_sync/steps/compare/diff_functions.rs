//! Function and procedure diff logic.

use std::collections::HashMap;

use suprim_sql::db::schema::FunctionNode;

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffKind, ObjectType};

/// Compare functions by signature (name + arg types) to handle overloads correctly.
pub(super) fn diff_functions(
    source: &[FunctionNode],
    target: &[FunctionNode],
    out: &mut Vec<DiffEntry>,
) {
    let src_map: HashMap<&str, &FunctionNode> =
        source.iter().map(|f| (f.signature.as_str(), f)).collect();
    let tgt_map: HashMap<&str, &FunctionNode> =
        target.iter().map(|f| (f.signature.as_str(), f)).collect();

    // Source-only → Added (create on target)
    for sig in sorted_keys(&src_map) {
        if !tgt_map.contains_key(sig) {
            let f = src_map[sig];
            let label = if f.is_procedure {
                "procedure"
            } else {
                "function"
            };
            out.push(DiffEntry {
                object_type: ObjectType::Function,
                name: sig.to_string(),
                detail: format!("{} ({})", label, f.language),
                kind: DiffKind::Added,
                checked: true,
                children: Vec::new(),
            });
        }
    }

    // Target-only → Removed (drop from target)
    for sig in sorted_keys(&tgt_map) {
        if !src_map.contains_key(sig) {
            let f = tgt_map[sig];
            let label = if f.is_procedure {
                "procedure"
            } else {
                "function"
            };
            out.push(DiffEntry {
                object_type: ObjectType::Function,
                name: sig.to_string(),
                detail: format!("{} ({})", label, f.language),
                kind: DiffKind::Removed,
                checked: true,
                children: Vec::new(),
            });
        }
    }

    // Both → compare definition body
    for sig in sorted_keys(&src_map) {
        if let (Some(src), Some(tgt)) = (src_map.get(sig), tgt_map.get(sig)) {
            let src_def = src.definition.trim();
            let tgt_def = tgt.definition.trim();
            if src_def != tgt_def || src.return_type != tgt.return_type {
                let label = if src.is_procedure {
                    "procedure"
                } else {
                    "function"
                };
                out.push(DiffEntry {
                    object_type: ObjectType::Function,
                    name: sig.to_string(),
                    detail: format!("{} ({}) — definition differs", label, src.language),
                    kind: DiffKind::Modified,
                    checked: true,
                    children: Vec::new(),
                });
            }
        }
    }
}

fn sorted_keys<'a, V>(map: &'a HashMap<&'a str, V>) -> Vec<&'a str> {
    let mut keys: Vec<&str> = map.keys().copied().collect();
    keys.sort();
    keys
}
