//! Structure Synchronization dialog — UI orchestration.
//!
//! Wires together the renderer modules and manages the egui::Window lifecycle.
//! State lives in `state.rs`, comparison logic in `steps/compare/`.

use eframe::egui;

use super::bottom_bar::render_bottom_bar;
use super::diff_results_renderer::{render_diff_results, render_loading_state};
use super::state::StructureSyncDialog;
use super::steps::select;
use super::types::{CompareRequest, CompareState, SyncDialogResult};

impl StructureSyncDialog {
    /// Render the dialog. Returns actions for the app to handle.
    pub fn show(&mut self, ctx: &egui::Context) -> SyncDialogResult {
        let mut open = true;
        let mut schema_requests = Vec::new();
        let mut database_requests = Vec::new();
        let mut connect_requests = Vec::new();
        let mut compare_request = None;

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
                if conn.databases.is_empty() {
                    let key = conn.conn_id;
                    if !self.pending_db_requests.contains(&key) {
                        self.pending_db_requests.insert(key);
                        database_requests.push(key);
                    }
                }
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

        const DIALOG_W: f32 = 720.0;
        const DIALOG_H: f32 = 480.0;

        egui::Window::new("Structure Synchronization")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([DIALOG_W, DIALOG_H])
            .show(ctx, |ui| {
                ui.set_min_height(DIALOG_H);
                ui.set_max_height(DIALOG_H);
                ui.set_min_width(DIALOG_W);

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

                // ── Top: pickers or summary ─────────────────────────────────
                match self.compare_state {
                    CompareState::Idle => {
                        select::render_endpoint_pickers(
                            ui,
                            &self.connections,
                            &mut self.source,
                            &mut self.target,
                        );
                    }
                    _ => {
                        select::render_endpoint_summary(
                            ui,
                            &self.connections,
                            &self.source,
                            &self.target,
                        );
                    }
                }

                ui.add_space(4.0);
                ui.separator();

                // ── Remaining area: middle + footer ─────────────────────────
                // Use bottom_up layout so footer renders at the actual bottom,
                // regardless of how much content the middle section has.
                let remaining = ui.available_size();
                let full_w = remaining.x;

                ui.allocate_ui_with_layout(
                    remaining,
                    egui::Layout::bottom_up(egui::Align::LEFT),
                    |ui| {
                        // Footer (rendered first because bottom_up)
                        let mut run_compare = false;
                        let mut reset_to_idle = false;
                        render_bottom_bar(
                            ui,
                            &self.compare_state,
                            &self.ddl_script,
                            &mut self.status,
                            &mut open,
                            &mut run_compare,
                            &mut reset_to_idle,
                        );
                        ui.add_space(4.0);
                        ui.separator();

                        // Middle content (rendered second = above footer)
                        let middle_h = ui.available_height();
                        ui.allocate_ui_with_layout(
                            egui::vec2(full_w, middle_h),
                            egui::Layout::top_down(egui::Align::LEFT),
                            |ui| match self.compare_state {
                                CompareState::Idle => {
                                    select::render_information_panels(
                                        ui,
                                        &self.connections,
                                        &self.source,
                                        &self.target,
                                        &self.status,
                                    );
                                }
                                CompareState::Loading => {
                                    render_loading_state(ui);
                                }
                                CompareState::Done => {
                                    render_diff_results(ui, &mut self.diff_groups);
                                    if let Some(status) = &self.status {
                                        ui.add_space(4.0);
                                        ui.label(egui::RichText::new(status).weak().size(11.0));
                                    }
                                }
                            },
                        );

                        if reset_to_idle {
                            self.compare_state = CompareState::Idle;
                            self.diff_groups.clear();
                            self.ddl_script.clear();
                            self.status = None;
                        } else if run_compare {
                            if let Some(req) = self.validate_and_create_compare_request() {
                                self.compare_state = CompareState::Loading;
                                self.diff_groups.clear();
                                self.ddl_script.clear();
                                self.status = Some("Loading schemas...".into());
                                compare_request = Some(req);
                            }
                        }
                    },
                );
            });

        SyncDialogResult {
            open,
            schema_requests,
            database_requests,
            connect_requests,
            compare_request,
        }
    }

    fn validate_and_create_compare_request(&mut self) -> Option<CompareRequest> {
        if self.source.database.is_empty() || self.target.database.is_empty() {
            self.status = Some("Please select a database for both source and target.".into());
            return None;
        }
        if self.source.schema.is_empty() || self.target.schema.is_empty() {
            self.status = Some("Please select a schema for both source and target.".into());
            return None;
        }

        let src_conn = self.connections.get(self.source.conn_idx)?;
        let tgt_conn = self.connections.get(self.target.conn_idx)?;

        if !src_conn.connected || !tgt_conn.connected {
            self.status = Some("Both connections must be active.".into());
            return None;
        }

        if src_conn.conn_id == tgt_conn.conn_id
            && self.source.database == self.target.database
            && self.source.schema == self.target.schema
        {
            self.status = Some("Source and target are the same schema.".into());
            return None;
        }

        Some(CompareRequest {
            source_conn_id: src_conn.conn_id,
            source_database: self.source.database.clone(),
            source_schema: self.source.schema.clone(),
            target_conn_id: tgt_conn.conn_id,
            target_database: self.target.database.clone(),
            target_schema: self.target.schema.clone(),
        })
    }
}
