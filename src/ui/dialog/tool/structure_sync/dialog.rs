//! Structure Synchronization dialog — UI orchestration.
//!
//! Wires together the renderer modules and manages the egui::Window lifecycle.
//! State lives in `state.rs`, comparison logic in `comparison.rs`.

use eframe::egui;

use super::state::StructureSyncDialog;
use super::steps::select;
use super::types::SyncDialogResult;

impl StructureSyncDialog {
    /// Render the dialog. Returns actions for the app to handle.
    pub fn show(&mut self, ctx: &egui::Context) -> SyncDialogResult {
        let mut open = true;
        let mut schema_requests = Vec::new();
        let mut database_requests = Vec::new();
        let mut connect_requests = Vec::new();

        // Check if selected endpoints need connecting, database, or schema loading
        for ep in [&self.source, &self.target] {
            if let Some(conn) = self.connections.get(ep.conn_idx) {
                if !conn.connected {
                    let key = conn.conn_id;
                    if !self.pending_db_requests.contains(&key) {
                        self.pending_db_requests.insert(key);
                        connect_requests.push(key);
                    }
                    continue;
                }
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

                select::render_header_banner(ui, &self.connections, &self.source, &self.target);
                ui.add_space(8.0);
                select::render_endpoint_pickers(
                    ui,
                    &self.connections,
                    &mut self.source,
                    &mut self.target,
                );
                ui.add_space(6.0);
                ui.separator();
                select::render_information_panels(
                    ui,
                    &self.connections,
                    &self.source,
                    &self.target,
                    &self.status,
                );
                ui.separator();
                ui.add_space(4.0);

                let mut run_compare = false;
                select::render_bottom_bar(
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
            connect_requests,
        }
    }
}
