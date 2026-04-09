//! Structure Synchronization dialog.
//!
//! Compares the schema structure (tables, columns, indexes, foreign keys)
//! between a **source** and **target** connection/database/schema, shows
//! information panels, and generates DDL to bring the target in sync.

use eframe::egui;

use super::structure_sync_renderer as renderer;
pub use super::structure_sync_types::{ConnInfo, ConnMeta, DbInfo, SyncDialogResult};
use super::structure_sync_types::{DiffEntry, DiffKind, Endpoint};

/// Top-level dialog state.
pub struct StructureSyncDialog {
    connections: Vec<ConnInfo>,
    source: Endpoint,
    target: Endpoint,
    diff_entries: Vec<DiffEntry>,
    ddl_script: String,
    compared: bool,
    status: Option<String>,
    /// Track which (conn_id, database) combos already had schema requests sent.
    pending_schema_requests: std::collections::HashSet<(uuid::Uuid, String)>,
    /// Track which conn_ids already had database list requests sent.
    pending_db_requests: std::collections::HashSet<uuid::Uuid>,
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

    /// Render the dialog. Returns actions for the app to handle.
    pub fn show(&mut self, ctx: &egui::Context) -> SyncDialogResult {
        let mut open = true;
        let mut schema_requests = Vec::new();
        let mut database_requests = Vec::new();

        // Check if selected endpoints need database or schema loading
        for ep in [&self.source, &self.target] {
            if let Some(conn) = self.connections.get(ep.conn_idx) {
                // Need databases?
                if conn.databases.is_empty() {
                    let key = conn.conn_id;
                    if !self.pending_db_requests.contains(&key) {
                        self.pending_db_requests.insert(key);
                        database_requests.push(key);
                    }
                }
                // Need schemas?
                if !ep.database.is_empty() {
                    let key = (conn.conn_id, ep.database.clone());
                    if !self.pending_schema_requests.contains(&key) {
                        let has_schemas = conn
                            .databases
                            .iter()
                            .find(|d| d.name == ep.database)
                            .map(|d| !d.schemas.is_empty())
                            .unwrap_or(false);
                        if !has_schemas {
                            self.pending_schema_requests.insert(key.clone());
                            schema_requests.push(key);
                        }
                    }
                }
            }
        }

        egui::Window::new("Structure Synchronization")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([660.0, 560.0])
            .show(ctx, |ui| {
                if self.connections.is_empty() {
                    ui.label("No active connections. Connect to a database first.");
                    ui.add_space(8.0);
                    if ui
                        .button("Close")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        open = false;
                    }
                    return;
                }

                renderer::render_header_banner(ui, &self.connections, &self.source, &self.target);
                ui.add_space(8.0);
                renderer::render_endpoint_pickers(
                    ui,
                    &self.connections,
                    &mut self.source,
                    &mut self.target,
                );
                ui.add_space(6.0);
                ui.separator();
                renderer::render_information_panels(
                    ui,
                    &self.connections,
                    &self.source,
                    &self.target,
                    &self.status,
                );
                ui.separator();
                ui.add_space(4.0);

                let mut run_compare = false;
                renderer::render_bottom_bar(
                    ui,
                    self.compared,
                    &self.ddl_script,
                    &mut self.status,
                    &mut open,
                    &mut run_compare,
                );
                if run_compare {
                    self.run_comparison();
                }
            });

        SyncDialogResult {
            open,
            schema_requests,
            database_requests,
        }
    }

    /// Update schema list for a connection+database when new data arrives.
    pub fn update_schemas(&mut self, conn_id: uuid::Uuid, database: &str, schemas: Vec<String>) {
        // Clear from pending set
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
        use super::structure_sync_types::DbInfo;

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
            }
        }
    }
}

// ── Comparison logic (placeholder) ──────────────────────────────────────

impl StructureSyncDialog {
    fn run_comparison(&mut self) {
        self.compared = true;
        self.diff_entries.clear();
        self.ddl_script.clear();
        self.status = None;

        if self.source.database.is_empty() || self.target.database.is_empty() {
            self.status = Some("Please select a database for both source and target.".into());
            self.compared = false;
            return;
        }
        if self.source.schema.is_empty() || self.target.schema.is_empty() {
            self.status = Some("Please select a schema for both source and target.".into());
            self.compared = false;
            return;
        }

        let src = match self.connections.get(self.source.conn_idx) {
            Some(c) => c,
            None => {
                self.status = Some("Invalid source connection.".into());
                self.compared = false;
                return;
            }
        };
        let tgt = match self.connections.get(self.target.conn_idx) {
            Some(c) => c,
            None => {
                self.status = Some("Invalid target connection.".into());
                self.compared = false;
                return;
            }
        };

        if src.conn_id == tgt.conn_id
            && self.source.database == self.target.database
            && self.source.schema == self.target.schema
        {
            self.status = Some("Source and target are the same schema.".into());
            self.compared = false;
            return;
        }

        // TODO: Real comparison via async schema fetching.
        self.status = Some(format!(
            "Comparison {}/{}/{} \u{2192} {}/{}/{} \u{2014} coming soon.",
            src.label,
            self.source.database,
            self.source.schema,
            tgt.label,
            self.target.database,
            self.target.schema,
        ));
    }

    #[allow(dead_code)]
    fn regenerate_script(&mut self) {
        let mut lines = Vec::new();
        for entry in &self.diff_entries {
            if !entry.checked {
                continue;
            }
            match entry.kind {
                DiffKind::Added => lines.push(format!("-- + {}", entry.label)),
                DiffKind::Removed => lines.push(format!("-- - {}", entry.label)),
                DiffKind::Modified => lines.push(format!("-- \u{0394} {}", entry.label)),
            }
        }
        self.ddl_script = lines.join("\n");
    }
}
