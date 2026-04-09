//! Structure Synchronization dialog — UI orchestration.
//!
//! Wires together the renderer modules and manages the egui::Window lifecycle.
//! State lives in `state.rs`, comparison logic in `steps/compare/`.

use eframe::egui;

use super::state::StructureSyncDialog;
use super::steps::select;
use super::types::{CompareRequest, CompareState, DiffEntry, DiffKind, SyncDialogResult};

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

                // Disable pickers while loading comparison
                let pickers_enabled = self.compare_state != CompareState::Loading;
                ui.add_enabled_ui(pickers_enabled, |ui| {
                    select::render_endpoint_pickers(
                        ui,
                        &self.connections,
                        &mut self.source,
                        &mut self.target,
                    );
                });

                ui.add_space(6.0);
                ui.separator();

                // Show diff results or info panels depending on compare state
                match self.compare_state {
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
                        render_diff_results(ui, &mut self.diff_entries, &self.ddl_script);
                        // Status line
                        if let Some(status) = &self.status {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new(status).weak().size(11.0));
                        }
                    }
                }

                ui.separator();
                ui.add_space(4.0);

                let mut run_compare = false;
                render_bottom_bar(
                    ui,
                    &self.compare_state,
                    &self.ddl_script,
                    &mut self.status,
                    &mut open,
                    &mut run_compare,
                );

                if run_compare {
                    // Validate selections first
                    if let Some(req) = self.validate_and_create_compare_request() {
                        self.compare_state = CompareState::Loading;
                        self.diff_entries.clear();
                        self.ddl_script.clear();
                        self.status = Some("Loading schemas...".into());
                        compare_request = Some(req);
                    }
                }
            });

        SyncDialogResult {
            open,
            schema_requests,
            database_requests,
            connect_requests,
            compare_request,
        }
    }

    /// Validate endpoint selections and build a CompareRequest if valid.
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

// ── Diff results rendering ──────────────────────────────────────────────────

fn render_loading_state(ui: &mut egui::Ui) {
    let avail = ui.available_size();
    ui.allocate_ui(egui::vec2(avail.x, avail.y.min(180.0)), |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.spinner();
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Comparing schemas...").weak());
        });
    });
}

fn render_diff_results(ui: &mut egui::Ui, entries: &mut [DiffEntry], _ddl_script: &str) {
    let avail = ui.available_size();
    let height = (avail.y - 60.0).max(100.0);

    egui::ScrollArea::vertical()
        .id_salt("diff_results")
        .max_height(height)
        .show(ui, |ui| {
            if entries.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(30.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "{}  Schemas are identical",
                            egui_phosphor::regular::CHECK_CIRCLE
                        ))
                        .size(14.0)
                        .color(egui::Color32::from_rgb(76, 175, 80)),
                    );
                });
                return;
            }

            for entry in entries.iter_mut() {
                let indent = entry.depth as f32 * 16.0;
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    ui.checkbox(&mut entry.checked, "");
                    let (icon, color) = match entry.kind {
                        DiffKind::Added => (
                            egui_phosphor::regular::PLUS_CIRCLE,
                            egui::Color32::from_rgb(76, 175, 80),
                        ),
                        DiffKind::Removed => (
                            egui_phosphor::regular::MINUS_CIRCLE,
                            egui::Color32::from_rgb(244, 67, 54),
                        ),
                        DiffKind::Modified => (
                            egui_phosphor::regular::PENCIL_SIMPLE,
                            egui::Color32::from_rgb(255, 152, 0),
                        ),
                    };
                    ui.label(egui::RichText::new(icon).color(color));
                    ui.label(&entry.label);
                });
            }
        });
}

// ── Bottom bar (extended for compare state) ─────────────────────────────────

fn render_bottom_bar(
    ui: &mut egui::Ui,
    compare_state: &CompareState,
    ddl_script: &str,
    status: &mut Option<String>,
    open: &mut bool,
    run_compare: &mut bool,
) {
    ui.horizontal(|ui| {
        if ui
            .button("Options")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            // TODO: options dialog
        }

        if *compare_state == CompareState::Done && !ddl_script.is_empty() {
            if ui
                .button(format!(
                    "{}  Copy Script",
                    egui_phosphor::regular::CLIPBOARD_TEXT
                ))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                ui.ctx().copy_text(ddl_script.to_owned());
                *status = Some("Script copied to clipboard".into());
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let compare_enabled = *compare_state != CompareState::Loading;
            let compare_label = match compare_state {
                CompareState::Loading => "Comparing...",
                _ => "Compare",
            };
            if ui
                .add_enabled(compare_enabled, egui::Button::new(compare_label))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *run_compare = true;
            }
            if ui
                .button("Close")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *open = false;
            }
        });
    });
}
