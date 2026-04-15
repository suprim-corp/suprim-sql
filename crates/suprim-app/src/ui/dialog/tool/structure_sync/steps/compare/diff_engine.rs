//! Schema diff engine — compares source and target `SchemaNode`s
//! and produces structured `DiffEntry` items.
//!
//! Sub-modules (siblings in compare/):
//! - `diff_tables` — tables, columns, indexes, foreign keys
//! - `diff_views_sequences` — views, materialized views, sequences
//! - `diff_functions` — functions and procedures
//! - `diff_extensions` — database-level extensions

use suprim_sql::db::schema::{ExtensionInfo, SchemaNode};

use crate::ui::dialog::tool::structure_sync::types::DiffEntry;

/// Compare two schemas and produce a flat list of top-level diff entries.
pub(crate) fn diff_schemas(source: &SchemaNode, target: &SchemaNode) -> Vec<DiffEntry> {
    let mut entries = Vec::new();

    super::diff_tables::diff_tables(&source.tables, &target.tables, &mut entries);
    super::diff_views_sequences::diff_views(false, &source.views, &target.views, &mut entries);
    super::diff_views_sequences::diff_views(
        true,
        &source.materialized_views,
        &target.materialized_views,
        &mut entries,
    );
    super::diff_views_sequences::diff_sequences(&source.sequences, &target.sequences, &mut entries);
    super::diff_functions::diff_functions(&source.functions, &target.functions, &mut entries);

    entries
}

/// Compare extensions between source and target databases (database-level objects).
/// This is separate from `diff_schemas` because extensions are not part of `SchemaNode`.
pub(crate) fn diff_extensions(
    source_extensions: &[ExtensionInfo],
    target_extensions: &[ExtensionInfo],
    entries: &mut Vec<DiffEntry>,
) {
    super::diff_extensions::diff_extensions(source_extensions, target_extensions, entries);
}
