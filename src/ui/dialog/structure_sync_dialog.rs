//! Structure Synchronization dialog.
//!
//! Compares the schema structure (tables, columns, indexes, foreign keys)
//! between a **source** and **target** connection/database/schema, shows
//! information panels, and generates DDL to bring the target in sync.

use eframe::egui;

use super::structure_sync_renderer as renderer;
pub use super::structure_sync_types::{ConnInfo, ConnMeta, DbInfo};
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

    /// Render the dialog. Returns `false` when the user closes it.
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut open = true;

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

        open
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
