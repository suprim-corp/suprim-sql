use std::collections::HashSet;

use suprim_sql::db::types::{DatabaseNode, SchemaNode, SchemaTree};
use uuid::Uuid;

/// Connection lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

/// A single connection entry shown in the sidebar.
pub(super) struct ConnectionEntry {
    pub conn_id: Uuid,
    pub label: String,
    pub status: ConnectionStatus,
    /// Currently displayed schema tree (already filtered).
    pub schema: Option<SchemaTree>,
    /// ALL databases returned from the server (unfiltered) -- for the picker.
    pub all_databases: Vec<DatabaseNode>,
    /// Which database names are visible. None = all.
    pub visible_databases: Option<Vec<String>>,
    /// When true, the next render will force-collapse this entry
    /// (resets egui's cached CollapsingState). Cleared after first render.
    pub needs_collapse: bool,
    /// Whether the db-picker popup is open.
    pub picker_open: bool,
    /// Schema names that have already had a LoadSchemaDetail request sent.
    pub schema_detail_requested: HashSet<String>,
    /// Database names that have already had a ListSchemas request sent.
    pub schemas_requested: HashSet<String>,
    /// Server version string (e.g. "PostgreSQL 16.2 on ...").
    pub server_version: Option<String>,
    /// Error message if connection failed.
    pub error_message: Option<String>,
    /// When true, the next render after connecting will force-expand this entry.
    /// Set when user clicks the chevron on a disconnected connection.
    pub needs_expand: bool,
}

impl ConnectionEntry {
    /// Create a fully-connected entry (existing behavior).
    pub fn new(
        conn_id: Uuid,
        label: String,
        schema: SchemaTree,
        visible_databases: Option<Vec<String>>,
    ) -> Self {
        let all_databases = schema.databases.clone();
        Self {
            conn_id,
            label,
            status: ConnectionStatus::Connected,
            schema: Some(schema),
            all_databases,
            visible_databases,
            needs_collapse: true,
            picker_open: false,
            schema_detail_requested: HashSet::new(),
            schemas_requested: HashSet::new(),
            server_version: None,
            error_message: None,
            needs_expand: false,
        }
    }

    /// Create a placeholder entry for a saved connection (not yet connected).
    pub fn new_disconnected(conn_id: Uuid, label: String) -> Self {
        Self {
            conn_id,
            label,
            status: ConnectionStatus::Disconnected,
            schema: None,
            all_databases: Vec::new(),
            visible_databases: None,
            needs_collapse: true,
            picker_open: false,
            schema_detail_requested: HashSet::new(),
            schemas_requested: HashSet::new(),
            server_version: None,
            error_message: None,
            needs_expand: false,
        }
    }

    /// Replace the full schema tree (e.g. after reconnect / refresh).
    pub fn replace_schema(&mut self, schema: SchemaTree) {
        self.all_databases = schema.databases.clone();
        self.schema = Some(schema);
        self.schema_detail_requested.clear();
        self.schemas_requested.clear();
    }

    /// Populate schema names for a specific database.
    pub fn set_schemas_for_database(&mut self, database: &str, schemas: Vec<String>) {
        let nodes: Vec<SchemaNode> = schemas
            .into_iter()
            .map(|name| SchemaNode {
                id: Uuid::new_v4(),
                name,
                loaded: false,
                tables: vec![],
                views: vec![],
                materialized_views: vec![],
                sequences: vec![],
                functions: vec![],
            })
            .collect();

        // Try the active (filtered) tree first, then all_databases.
        if let Some(tree) = &mut self.schema {
            if let Some(db) = tree.databases.iter_mut().find(|d| d.name == database) {
                db.schemas = nodes;
                return;
            }
        }
        if let Some(db) = self.all_databases.iter_mut().find(|d| d.name == database) {
            db.schemas = nodes;
        }
    }

    /// Replace a single schema node with fully loaded detail.
    pub fn set_schema_detail(&mut self, database: &str, schema_name: &str, detail: SchemaNode) {
        if let Some(tree) = &mut self.schema {
            for db in &mut tree.databases {
                if db.name != database {
                    continue;
                }
                if let Some(schema) = db.schemas.iter_mut().find(|s| s.name == schema_name) {
                    *schema = detail;
                    self.schema_detail_requested
                        .remove(&format!("{database}:{schema_name}"));
                    return;
                }
            }
        }
    }
}
