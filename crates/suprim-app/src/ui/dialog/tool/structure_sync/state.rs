//! StructureSyncDialog state, construction, and event-driven data updates.

use suprim_core::db::schema::{ExtensionInfo, SchemaNode};

use super::steps::compare::{ddl_generator, diff_engine};
use super::types::{CompareState, ConnInfo, DbInfo, DiffGroup, DiffKind, Endpoint, WizardStep};

/// Top-level dialog state.
pub struct StructureSyncDialog {
    pub(super) connections: Vec<ConnInfo>,
    pub(super) source: Endpoint,
    pub(super) target: Endpoint,
    #[allow(dead_code)]
    pub(super) step: WizardStep,
    /// Three groups: Modified, Created, Deleted.
    pub(super) diff_groups: Vec<DiffGroup>,
    pub(super) ddl_script: String,
    pub(super) compare_state: CompareState,
    pub(super) status: Option<String>,
    /// Track which (conn_id, database) combos already had schema requests sent.
    pub(super) pending_schema_requests: std::collections::HashSet<(uuid::Uuid, String)>,
    /// Track which conn_ids already had database list requests sent.
    pub(super) pending_db_requests: std::collections::HashSet<uuid::Uuid>,
    /// Cached source schema node (from last comparison).
    pub(super) source_schema_node: Option<SchemaNode>,
    /// Cached target schema node (from last comparison).
    pub(super) target_schema_node: Option<SchemaNode>,
    /// Extensions installed on source database.
    pub(super) source_extensions: Vec<ExtensionInfo>,
    /// Extensions installed on target database.
    pub(super) target_extensions: Vec<ExtensionInfo>,
}

// ── Construction ────────────────────────────────────────────────────────

impl StructureSyncDialog {
    pub fn new(connections: Vec<ConnInfo>) -> Self {
        let source = Self::default_endpoint(&connections, 0);
        let target =
            Self::default_endpoint(&connections, if connections.len() > 1 { 1 } else { 0 });

        Self {
            connections,
            source,
            target,
            step: WizardStep::default(),
            diff_groups: Vec::new(),
            ddl_script: String::new(),
            compare_state: CompareState::default(),
            status: None,
            pending_schema_requests: std::collections::HashSet::new(),
            pending_db_requests: std::collections::HashSet::new(),
            source_schema_node: None,
            target_schema_node: None,
            source_extensions: Vec::new(),
            target_extensions: Vec::new(),
        }
    }

    fn default_endpoint(connections: &[ConnInfo], idx: usize) -> Endpoint {
        if let Some(conn) = connections.get(idx) {
            let database = conn
                .databases
                .first()
                .map(|d| d.name.clone())
                .unwrap_or_default();
            let schema = conn
                .databases
                .first()
                .and_then(|d| d.schemas.first().cloned())
                .unwrap_or_default();
            Endpoint {
                conn_idx: idx,
                database,
                schema,
            }
        } else {
            Endpoint::default()
        }
    }
}

// ── Event-driven data updates ───────────────────────────────────────────

impl StructureSyncDialog {
    /// Update schema list for a connection+database when new data arrives.
    pub fn update_schemas(&mut self, conn_id: uuid::Uuid, database: &str, schemas: Vec<String>) {
        self.pending_schema_requests
            .remove(&(conn_id, database.to_string()));

        for conn in &mut self.connections {
            if conn.conn_id == conn_id {
                if let Some(db) = conn.databases.iter_mut().find(|d| d.name == database) {
                    db.schemas = schemas.clone();
                }
            }
        }
        // Auto-select first schema if endpoint's schema is empty
        for ep in [&mut self.source, &mut self.target] {
            if let Some(conn) = self.connections.get(ep.conn_idx) {
                if conn.conn_id == conn_id && ep.database == database && ep.schema.is_empty() {
                    if let Some(db) = conn.databases.iter().find(|d| d.name == database) {
                        ep.schema = db.schemas.first().cloned().unwrap_or_default();
                    }
                }
            }
        }
    }

    /// Update database list for a connection when new data arrives.
    pub fn update_databases(&mut self, conn_id: uuid::Uuid, databases: Vec<String>) {
        self.pending_db_requests.remove(&conn_id);

        for conn in &mut self.connections {
            if conn.conn_id == conn_id {
                conn.databases = databases
                    .iter()
                    .map(|name| DbInfo {
                        name: name.clone(),
                        schemas: Vec::new(),
                    })
                    .collect();
            }
        }
        // Auto-select first database if endpoint's database is empty
        for ep in [&mut self.source, &mut self.target] {
            if let Some(conn) = self.connections.get(ep.conn_idx) {
                if conn.conn_id == conn_id && ep.database.is_empty() {
                    ep.database = conn
                        .databases
                        .first()
                        .map(|d| d.name.clone())
                        .unwrap_or_default();
                }
            }
        }
    }

    /// Update server version for a connection when Connected event arrives.
    pub fn update_server_version(&mut self, conn_id: uuid::Uuid, version: Option<String>) {
        for conn in &mut self.connections {
            if conn.conn_id == conn_id {
                conn.meta.server_version = version.clone();
                conn.connected = true;
            }
        }
    }

    /// Called when the DB worker returns both schema nodes + extensions.
    /// Runs diff + DDL generation on the UI thread (fast, pure Rust).
    pub fn on_schemas_compared(
        &mut self,
        source: SchemaNode,
        target: SchemaNode,
        source_extensions: Vec<ExtensionInfo>,
        target_extensions: Vec<ExtensionInfo>,
    ) {
        let mut all_entries = diff_engine::diff_schemas(&source, &target);
        // Diff extensions (database-level objects)
        diff_engine::diff_extensions(&source_extensions, &target_extensions, &mut all_entries);

        // Group entries by DiffKind into 3 groups (Modified, Created, Deleted).
        let modified: Vec<_> = all_entries
            .iter()
            .filter(|e| e.kind == DiffKind::Modified)
            .cloned()
            .collect();
        let added: Vec<_> = all_entries
            .iter()
            .filter(|e| e.kind == DiffKind::Added)
            .cloned()
            .collect();
        let removed: Vec<_> = all_entries
            .into_iter()
            .filter(|e| e.kind == DiffKind::Removed)
            .collect();

        self.diff_groups = vec![
            DiffGroup {
                kind: DiffKind::Modified,
                entries: modified,
            },
            DiffGroup {
                kind: DiffKind::Added,
                entries: added,
            },
            DiffGroup {
                kind: DiffKind::Removed,
                entries: removed,
            },
        ];

        self.ddl_script = ddl_generator::generate_ddl(
            &source,
            &target,
            &self.target.schema,
            &self.diff_groups,
            &source_extensions,
            &target_extensions,
        );
        self.source_schema_node = Some(source);
        self.target_schema_node = Some(target);
        self.source_extensions = source_extensions;
        self.target_extensions = target_extensions;
        self.compare_state = CompareState::Done;

        let total: usize = self.diff_groups.iter().map(|g| g.total_count()).sum();
        if total == 0 {
            self.status = Some("Schemas are identical — no differences found.".into());
        } else {
            let added = self.diff_groups[1].total_count();
            let removed = self.diff_groups[2].total_count();
            let modified = self.diff_groups[0].total_count();
            self.status = Some(format!(
                "Found {} difference(s): {} added, {} removed, {} modified",
                total, added, removed, modified,
            ));
        }
    }

    /// Regenerate DDL script from current (possibly toggled) diff entries.
    pub(super) fn regenerate_script(&mut self) {
        if let (Some(src), Some(tgt)) = (&self.source_schema_node, &self.target_schema_node) {
            self.ddl_script = ddl_generator::generate_ddl(
                src,
                tgt,
                &self.target.schema,
                &self.diff_groups,
                &self.source_extensions,
                &self.target_extensions,
            );
        }
    }
}
