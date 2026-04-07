/// Schema tree model — sidebar data structure for database/schema/table browsing.
/// Separated from value types (`values.rs`) since these serve different concerns.

/// Full schema tree — root of the sidebar model
#[derive(Debug, Clone, Default)]
pub struct SchemaTree {
    pub databases: Vec<DatabaseNode>,
}

#[derive(Debug, Clone)]
pub struct DatabaseNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub schemas: Vec<SchemaNode>,
}

#[derive(Debug, Clone)]
pub struct SchemaNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub tables: Vec<TableNode>,
    pub views: Vec<ViewNode>,
    pub materialized_views: Vec<ViewNode>,
    pub sequences: Vec<SequenceNode>,
    /// Whether table/view detail has been loaded (for lazy loading in UI).
    pub loaded: bool,
}

#[derive(Debug, Clone)]
pub struct TableNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub columns: Vec<ColumnNode>,
    pub indexes: Vec<IndexNode>,
    pub foreign_keys: Vec<ForeignKeyNode>,
    pub row_count: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ViewNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub columns: Vec<ColumnNode>,
}

#[derive(Debug, Clone)]
pub struct ColumnNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub db_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IndexNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
}

#[derive(Debug, Clone)]
pub struct SequenceNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub data_type: String,
    pub start_value: i64,
    pub increment: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub last_value: Option<i64>,
    /// Table.column that owns this sequence (e.g. "users.id"), if any.
    pub owner: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ForeignKeyNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_tree_default_is_empty() {
        let tree = SchemaTree::default();
        assert!(tree.databases.is_empty());
    }
}
