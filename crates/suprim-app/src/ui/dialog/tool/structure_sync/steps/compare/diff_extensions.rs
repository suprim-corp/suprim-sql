//! Extension diff — compares installed extensions between source and target databases.

use std::collections::BTreeSet;

use suprim_core::db::schema::ExtensionInfo;

use crate::ui::dialog::tool::structure_sync::types::{DiffEntry, DiffKind, ObjectType};

/// Compare extensions between source and target databases.
/// Extensions present on source but not target → Added (need to be installed on target).
/// Extensions present on target but not source → Removed (would be dropped from target).
pub(crate) fn diff_extensions(
    source: &[ExtensionInfo],
    target: &[ExtensionInfo],
    entries: &mut Vec<DiffEntry>,
) {
    let src_names: BTreeSet<&str> = source.iter().map(|e| e.name.as_str()).collect();
    let tgt_names: BTreeSet<&str> = target.iter().map(|e| e.name.as_str()).collect();

    // Extensions in source but not target → need to be created on target
    for ext in source {
        if !tgt_names.contains(ext.name.as_str()) {
            entries.push(DiffEntry {
                object_type: ObjectType::Extension,
                name: ext.name.clone(),
                detail: format!("v{}", ext.version),
                kind: DiffKind::Added,
                checked: true,
                children: Vec::new(),
            });
        }
    }

    // Extensions in target but not source → would be dropped from target
    for ext in target {
        if !src_names.contains(ext.name.as_str()) {
            entries.push(DiffEntry {
                object_type: ObjectType::Extension,
                name: ext.name.clone(),
                detail: format!("v{}", ext.version),
                kind: DiffKind::Removed,
                checked: true,
                children: Vec::new(),
            });
        }
    }

    // Version differences → Modified
    for src_ext in source {
        if let Some(tgt_ext) = target.iter().find(|e| e.name == src_ext.name) {
            if src_ext.version != tgt_ext.version {
                entries.push(DiffEntry {
                    object_type: ObjectType::Extension,
                    name: src_ext.name.clone(),
                    detail: format!("v{} → v{}", tgt_ext.version, src_ext.version),
                    kind: DiffKind::Modified,
                    checked: true,
                    children: Vec::new(),
                });
            }
        }
    }
}
