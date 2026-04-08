mod connection_entry;
mod database_picker;
mod schema_renderer;
mod sequences_folder_renderer;
mod sidebar_action;
mod table_context_menu;
mod table_detail_renderer;
mod tables_folder_renderer;
mod view_detail_renderer;
mod views_folder_renderer;

use connection_entry::ConnectionEntry;
use eframe::egui::{self, CursorIcon};
use suprim_sql::db::types::{SchemaNode, SchemaTree};
use uuid::Uuid;

pub use sidebar_action::SidebarAction;

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

    pub fn active_connection_ids(&self) -> Vec<Uuid> {
        self.connections.iter().map(|c| c.conn_id).collect()
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

    // ─── State mutations (delegated to ConnectionEntry) ─────────────────

    pub fn on_connected(
        &mut self,
        conn_id: Uuid,
        name: String,
        schema: SchemaTree,
        visible: Option<Vec<String>>,
    ) {
        self.connections.retain(|c| c.conn_id != conn_id);
        self.connections
            .push(ConnectionEntry::new(conn_id, name, schema, visible));
    }

    pub fn on_disconnected(&mut self, conn_id: Uuid) {
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

    // ─── Rendering ──────────────────────────────────────────────────────

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<SidebarAction> {
        let mut action: Option<SidebarAction> = None;
        let mut disconnect_id: Option<Uuid> = None;

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for entry in &mut self.connections {
                    let conn_id = entry.conn_id;
                    let label = truncate_label(&entry.label, 24);
                    let header = build_header_label(
                        &label,
                        entry.all_databases.len(),
                        &entry.visible_databases,
                    );

                    let resp = egui::CollapsingHeader::new(&header)
                        .default_open(entry.expanded)
                        .id_salt(conn_id)
                        .show(ui, |ui| {
                            if let Some(schema) = &entry.schema {
                                if let Some(a) = schema_renderer::render_schema_tree(
                                    ui,
                                    conn_id,
                                    schema,
                                    entry.visible_databases.as_ref(),
                                    &mut entry.schema_detail_requested,
                                    &mut entry.schemas_requested,
                                ) {
                                    action = Some(a);
                                }
                            }
                        });

                    render_context_menu(
                        &resp.header_response,
                        conn_id,
                        entry,
                        &mut action,
                        &mut disconnect_id,
                    );
                    resp.header_response
                        .on_hover_cursor(CursorIcon::PointingHand);
                    render_database_picker(ui, conn_id, entry, &mut action);
                }
            });

        if let Some(id) = disconnect_id {
            action = Some(SidebarAction::Disconnect { conn_id: id });
        }
        action
    }

    // ─── Private helpers ────────────────────────────────────────────────

    fn find(&self, conn_id: Uuid) -> Option<&ConnectionEntry> {
        self.connections.iter().find(|c| c.conn_id == conn_id)
    }

    fn find_mut(&mut self, conn_id: Uuid) -> Option<&mut ConnectionEntry> {
        self.connections.iter_mut().find(|c| c.conn_id == conn_id)
    }
}

// ─── Free functions ─────────────────────────────────────────────────────────

fn render_context_menu(
    header: &egui::Response,
    conn_id: Uuid,
    entry: &mut ConnectionEntry,
    action: &mut Option<SidebarAction>,
    disconnect_id: &mut Option<Uuid>,
) {
    header.context_menu(|ui| {
        if ui
            .button("New SQL Tab")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            let databases: Vec<String> =
                entry.all_databases.iter().map(|d| d.name.clone()).collect();
            *action = Some(SidebarAction::OpenSqlTab {
                conn_id,
                database: databases.first().cloned(),
                databases,
            });
            ui.close();
        }
        ui.separator();
        if ui
            .button("Filter Databases...")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            entry.picker_open = !entry.picker_open;
            ui.close();
        }
        if ui
            .button("Edit Connection...")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::EditConnection { conn_id });
            ui.close();
        }
        if ui
            .button("Disconnect")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            *disconnect_id = Some(conn_id);
            ui.close();
        }
    });
}

fn render_database_picker(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    entry: &mut ConnectionEntry,
    action: &mut Option<SidebarAction>,
) {
    if !entry.picker_open {
        return;
    }
    let label = truncate_label(&entry.label, 24);
    let picker_id = egui::Id::new(format!("db_picker_{conn_id}"));
    if let Some(new_visible) = database_picker::render_database_picker(
        ui.ctx(),
        &label,
        picker_id,
        &mut entry.picker_open,
        &entry.all_databases,
        &entry.visible_databases,
    ) {
        entry.visible_databases = new_visible.clone();
        if action.is_none() {
            *action = Some(SidebarAction::UpdateVisibleDatabases {
                conn_id,
                visible: new_visible,
            });
        }
    }
}

fn truncate_label(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", chars[..max_chars - 1].iter().collect::<String>())
    }
}

fn build_header_label(label: &str, total: usize, visible: &Option<Vec<String>>) -> String {
    let badge = match visible {
        Some(v) => format!("{}/{}", v.len(), total),
        None => total.to_string(),
    };
    format!("{}  [{}]", label, badge)
}
