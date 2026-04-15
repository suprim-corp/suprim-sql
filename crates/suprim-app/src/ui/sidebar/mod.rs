mod connection_entry;
mod context_menus;
mod database_picker;
mod schema_renderer;
mod sequences_folder_renderer;
mod sidebar_action;
mod sidebar_renderer;
mod table_context_menu;
mod table_detail_renderer;
mod tables_folder_renderer;
mod view_detail_renderer;
mod views_folder_renderer;

use connection_entry::{ConnectionEntry, ConnectionStatus};
use eframe::egui;
use suprim_sql::db::connection::ConnectionConfig;
use suprim_sql::db::types::{SchemaNode, SchemaTree};
use uuid::Uuid;

pub use sidebar_action::SidebarAction;

/// Connection info entry for dialog dropdowns: (conn_id, label, databases_with_schemas, server_version, connected).
pub type ConnListEntry = (
    Uuid,
    String,
    Vec<(String, Vec<String>)>,
    Option<String>,
    bool,
);

/// The left-hand schema / connection browser panel.
pub struct Sidebar {
    connections: Vec<ConnectionEntry>,
}

impl Sidebar {
    pub fn new() -> Self {
        Self {
            connections: Vec::new(),
        }
    }

    /// Pre-populate sidebar with all saved connections in "Disconnected" state.
    pub fn init_from_config(&mut self, configs: &[ConnectionConfig]) {
        for cfg in configs {
            if !self.connections.iter().any(|c| c.conn_id == cfg.id) {
                self.connections
                    .push(ConnectionEntry::new_disconnected(cfg.id, cfg.name.clone()));
            }
        }
    }

    pub fn active_connection_ids(&self) -> Vec<Uuid> {
        self.connections.iter().map(|c| c.conn_id).collect()
    }

    /// Returns a list of connection info entries for all active connections.
    /// Each database entry is (db_name, vec_of_schema_names).
    /// Used by dialogs that need connection/database/schema dropdowns.
    pub fn connection_list(&self) -> Vec<ConnListEntry> {
        self.connections
            .iter()
            .map(|c| {
                let dbs: Vec<(String, Vec<String>)> = c
                    .all_databases
                    .iter()
                    .map(|d| {
                        let schemas = d.schemas.iter().map(|s| s.name.clone()).collect();
                        (d.name.clone(), schemas)
                    })
                    .collect();
                let connected = c.status == ConnectionStatus::Connected;
                (
                    c.conn_id,
                    c.label.clone(),
                    dbs,
                    c.server_version.clone(),
                    connected,
                )
            })
            .collect()
    }

    /// Returns (conn_id, conn_name, first_database, all_databases) for the
    /// first active connection, if any. Used by menu bar "New SQL Tab".
    pub fn first_connection_info(&self) -> Option<(Uuid, String, Option<String>, Vec<String>)> {
        let entry = self.connections.first()?;
        let databases: Vec<String> = entry.all_databases.iter().map(|d| d.name.clone()).collect();
        Some((
            entry.conn_id,
            entry.label.clone(),
            databases.first().cloned(),
            databases,
        ))
    }

    pub fn conn_name(&self, conn_id: Uuid) -> String {
        self.find(conn_id)
            .map(|c| c.label.clone())
            .unwrap_or_default()
    }

    // ─── State mutations ────────────────────────────────────────────────

    pub fn on_connected(
        &mut self,
        conn_id: Uuid,
        name: String,
        schema: SchemaTree,
        visible: Option<Vec<String>>,
        server_version: Option<String>,
    ) {
        if let Some(entry) = self.find_mut(conn_id) {
            // Update existing entry in place
            entry.label = name;
            entry.status = ConnectionStatus::Connected;
            entry.all_databases = schema.databases.clone();
            entry.visible_databases = visible;
            entry.schema = Some(schema);
            entry.server_version = server_version;
            entry.error_message = None;
            entry.schema_detail_requested.clear();
            entry.schemas_requested.clear();
        } else {
            // New connection (not from config)
            let mut entry = ConnectionEntry::new(conn_id, name, schema, visible);
            entry.server_version = server_version;
            self.connections.push(entry);
        }
    }

    /// Mark a connection as failed (e.g. connect error).
    pub fn on_connect_failed(&mut self, conn_id: Uuid, error: String) {
        if let Some(entry) = self.find_mut(conn_id) {
            entry.status = ConnectionStatus::Failed;
            entry.error_message = Some(error);
        }
    }

    /// Mark a connection as "connecting" (connect attempt started).
    pub fn on_connecting(&mut self, conn_id: Uuid) {
        if let Some(entry) = self.find_mut(conn_id) {
            entry.status = ConnectionStatus::Connecting;
            entry.error_message = None;
        }
    }

    pub fn on_disconnected(&mut self, conn_id: Uuid) {
        if let Some(entry) = self.find_mut(conn_id) {
            entry.status = ConnectionStatus::Disconnected;
            entry.schema = None;
            entry.all_databases.clear();
            entry.server_version = None;
            entry.error_message = None;
            entry.schema_detail_requested.clear();
            entry.schemas_requested.clear();
        }
    }

    /// Remove a connection entry entirely (e.g. user deletes from config).
    pub fn remove_connection(&mut self, conn_id: Uuid) {
        self.connections.retain(|c| c.conn_id != conn_id);
    }

    pub fn on_schema_loaded(&mut self, conn_id: Uuid, schema: SchemaTree) {
        if let Some(entry) = self.find_mut(conn_id) {
            entry.replace_schema(schema);
        }
    }

    pub fn on_schemas_listed(&mut self, conn_id: Uuid, database: &str, schemas: Vec<String>) {
        if let Some(entry) = self.find_mut(conn_id) {
            entry.set_schemas_for_database(database, schemas);
        }
    }

    pub fn on_schema_detail_loaded(
        &mut self,
        conn_id: Uuid,
        database: &str,
        schema_name: &str,
        detail: SchemaNode,
    ) {
        if let Some(entry) = self.find_mut(conn_id) {
            entry.set_schema_detail(database, schema_name, detail);
        }
    }

    /// Mark a connection so it auto-expands once connected (workspace restore).
    pub fn mark_needs_expand(&mut self, conn_id: Uuid) {
        if let Some(entry) = self.find_mut(conn_id) {
            entry.needs_expand = true;
        }
    }

    /// Return IDs of all currently connected connections (for workspace persistence).
    pub fn connected_ids(&self) -> Vec<Uuid> {
        self.connections
            .iter()
            .filter(|c| c.status == ConnectionStatus::Connected)
            .map(|c| c.conn_id)
            .collect()
    }

    // ─── Rendering ──────────────────────────────────────────────────────

    /// Render the sidebar. `connection_limit` is `Some(max)` for free tier, `None` for unlimited.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        connection_limit: Option<usize>,
    ) -> Option<SidebarAction> {
        // Connection count header
        let count = self.connections.len();
        ui.horizontal(|ui| {
            let header = match connection_limit {
                Some(max) => format!(
                    "{} Connections ({}/{})",
                    egui_phosphor::regular::PLUGS_CONNECTED,
                    count,
                    max
                ),
                None => format!(
                    "{} Connections ({})",
                    egui_phosphor::regular::PLUGS_CONNECTED,
                    count
                ),
            };
            ui.label(egui::RichText::new(header).size(12.0).weak());
        });
        ui.add_space(2.0);

        sidebar_renderer::render_connections(ui, &mut self.connections)
    }

    // ─── Private helpers ────────────────────────────────────────────────

    fn find(&self, conn_id: Uuid) -> Option<&ConnectionEntry> {
        self.connections.iter().find(|c| c.conn_id == conn_id)
    }

    fn find_mut(&mut self, conn_id: Uuid) -> Option<&mut ConnectionEntry> {
        self.connections.iter_mut().find(|c| c.conn_id == conn_id)
    }
}
