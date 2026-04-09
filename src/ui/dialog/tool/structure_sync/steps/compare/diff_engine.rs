//! Schema diff engine — compares source and target `SchemaNode`s
//! and produces a flat list of `DiffEntry` items.
//!
//! Sub-modules handle specific object types:
//! - `diff_tables` — tables, columns, indexes, foreign keys
//! - `diff_views_sequences` — views, materialized views, sequences

use suprim_sql::db::schema::SchemaNode;

use crate::ui::dialog::tool::structure_sync::types::DiffEntry;

/// Compare two schemas and produce a flat diff list.
pub(crate) fn diff_schemas(source: &SchemaNode, target: &SchemaNode) -> Vec<DiffEntry> {
    let mut entries = Vec::new();

    super::diff_tables::diff_tables(&source.tables, &target.tables, &mut entries);
    super::diff_views_sequences::diff_views("View", &source.views, &target.views, &mut entries);
    super::diff_views_sequences::diff_views(
        "Materialized View",
        &source.materialized_views,
        &target.materialized_views,
        &mut entries,
    );
    super::diff_views_sequences::diff_sequences(&source.sequences, &target.sequences, &mut entries);

    entries
}
