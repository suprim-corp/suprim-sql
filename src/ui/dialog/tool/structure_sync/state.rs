//! StructureSyncDialog state, construction, and event-driven data updates.

use super::types::{ConnInfo, DbInfo, DiffEntry, Endpoint, WizardStep};

/// Top-level dialog state.
pub struct StructureSyncDialog {
    pub(super) connections: Vec<ConnInfo>,
    pub(super) source: Endpoint,
    pub(super) target: Endpoint,
    #[allow(dead_code)]
    pub(super) step: WizardStep,
    pub(super) diff_entries: Vec<DiffEntry>,
    pub(super) ddl_script: String,
    pub(super) compared: bool,
    pub(super) status: Option<String>,
    /// Track which (conn_id, database) combos already had schema requests sent.
    pub(super) pending_schema_requests: std::collections::HashSet<(uuid::Uuid, String)>,
    /// Track which conn_ids already had database list requests sent.
    pub(super) pending_db_requests: std::collections::HashSet<uuid::Uuid>,
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
            diff_entries: Vec::new(),
            ddl_script: String::new(),
            compared: false,
            status: None,
            pending_schema_requests: std::collections::HashSet::new(),
            pending_db_requests: std::collections::HashSet::new(),
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
}
