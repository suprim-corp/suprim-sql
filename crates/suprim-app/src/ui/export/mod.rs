//! Export dialog — TablePro-style 2-column UI.
//!
//! Left pane: tables tree with tristate checkboxes (only in Tables mode).
//! Right pane: format picker + format-specific options + file name.
//!
//! Files in this module:
//! - `types.rs`         — shared enums/structs (ExportFormatId, FormatOptions, PendingExport, …)
//! - `csv_options.rs`   — CSV options + UI
//! - `json_options.rs`  — JSON options + UI
//! - `sql_options.rs`   — SQL options + UI
//! - `tree_widgets.rs`  — tristate checkbox + tree node rendering
//! - `dialog_ui.rs`     — options panel + tree scroll container
//! - `build.rs`         — validation + request build (native save dialog)
//! - `writers/`         — CSV, JSON, SQL file writers + shared dispatch

mod build;
pub mod csv_options;
mod dialog_ui;
pub mod json_options;
pub mod sql_options;
mod tree_widgets;
pub mod types;
pub mod writers;

use eframe::egui;
use suprim_core::db::dialect::SqlDialect;

pub use csv_options::CsvOptions;
pub use json_options::JsonOptions;
pub use sql_options::SqlOptions;
pub use types::{
    ExportDatabaseItem, ExportFormatId, ExportMode, ExportModeKind, ExportOutcome, ExportRequest,
    ExportSchemaItem, ExportTableItem, PendingExport,
};

// ── Dialog state ────────────────────────────────────────────────────────────

pub struct ExportDialog {
    pub(super) mode: ExportMode,
    pub(super) format: ExportFormatId,
    pub(super) file_name: String,
    pub(super) csv_opts: CsvOptions,
    pub(super) json_opts: JsonOptions,
    pub(super) sql_opts: SqlOptions,
    pub(super) error: Option<String>,
    /// SQL dialect for the connection this export targets.
    pub(super) dialect: SqlDialect,
}

impl ExportDialog {
    /// Open in "Tables" mode — user picks tables + format, app fetches + exports.
    pub fn for_tables(
        conn_id: uuid::Uuid,
        items: Vec<ExportDatabaseItem>,
        default_name: String,
        dialect: SqlDialect,
    ) -> Self {
        Self {
            mode: ExportMode::Tables { conn_id, items },
            format: ExportFormatId::Csv,
            file_name: default_name,
            csv_opts: CsvOptions::default(),
            json_opts: JsonOptions::default(),
            sql_opts: SqlOptions::default(),
            error: None,
            dialect,
        }
    }

    /// Open in "QueryResult" mode — data already in memory.
    pub fn for_query_result(
        suggested_name: String,
        result: suprim_core::db::values::QueryResult,
    ) -> Self {
        Self {
            mode: ExportMode::QueryResult { result },
            format: ExportFormatId::Csv,
            file_name: suggested_name,
            csv_opts: CsvOptions::default(),
            json_opts: JsonOptions::default(),
            sql_opts: SqlOptions::default(),
            error: None,
            dialect: SqlDialect::default(),
        }
    }

    /// Render one frame of the dialog. Returns the outcome.
    pub fn show(&mut self, ctx: &egui::Context) -> ExportOutcome {
        let mut outcome = ExportOutcome::Pending;
        let mut is_open = true;

        let is_tables_mode = matches!(self.mode, ExportMode::Tables { .. });
        let is_sql = self.format == ExportFormatId::Sql;
        let dialog_width = if is_tables_mode { 920.0 } else { 460.0 };
        let dialog_height = 480.0;

        #[allow(unused_mut)]
        let mut window = egui::Window::new("Export")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([dialog_width, dialog_height]);

        #[cfg(target_os = "macos")]
        {
            window = window.title_bar(false);
        }

        // On non-macOS: use a separate bool for the title bar close button to
        // avoid double mutable borrow of `is_open` (closure also needs it).
        #[cfg(not(target_os = "macos"))]
        let mut title_bar_open = true;
        #[cfg(not(target_os = "macos"))]
        {
            window = window.open(&mut title_bar_open);
        }

        window.show(ctx, |ui| {
            #[cfg(target_os = "macos")]
            self.render_macos_title_bar(ui, &mut is_open);

            // Validate up-front so the error message is in sync with the current state.
            let can_export = self.validate_state();

            // ── Body ─────────────────────────────────────────────────
            const FOOTER_H: f32 = 44.0;
            let body_h = (ui.available_height() - FOOTER_H).max(100.0);

            if is_tables_mode {
                let target_w = if is_sql { 480.0 } else { 380.0 };
                let left_pane_width = ui.ctx().animate_value_with_time(
                    egui::Id::new("export_left_pane_w"),
                    target_w,
                    0.15,
                );
                ui.horizontal(|ui| {
                    ui.set_height(body_h);
                    // LEFT: tables tree
                    ui.vertical(|ui| {
                        ui.set_min_width(left_pane_width);
                        ui.set_max_width(left_pane_width);
                        // Header row: "Items" left, SQL column headers right
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Items").strong());
                            if is_sql {
                                dialog_ui::render_sql_column_headers(ui);
                            }
                        });
                        ui.add_space(4.0);
                        self.render_tables_tree(ui, body_h - 30.0);
                    });
                    ui.separator();
                    // RIGHT: options
                    ui.vertical(|ui| {
                        self.render_options_panel(ui);
                    });
                });
            } else {
                ui.vertical(|ui| {
                    ui.set_height(body_h);
                    self.render_options_panel(ui);
                });
            }

            // ── Footer ───────────────────────────────────────────────
            ui.separator();
            self.render_footer(ui, can_export, is_tables_mode, &mut is_open, &mut outcome);
        });

        // Merge title bar close into is_open
        #[cfg(not(target_os = "macos"))]
        if !title_bar_open {
            is_open = false;
        }

        if !is_open && matches!(outcome, ExportOutcome::Pending) {
            return ExportOutcome::Cancelled;
        }
        outcome
    }

    #[cfg(target_os = "macos")]
    fn render_macos_title_bar(&self, ui: &mut egui::Ui, is_open: &mut bool) {
        ui.horizontal(|ui| {
            let radius = 6.0;
            let (dot_rect, resp) = ui
                .allocate_exact_size(egui::vec2(radius * 2.0, radius * 2.0), egui::Sense::click());
            let color = if resp.hovered() {
                egui::Color32::from_rgb(255, 80, 80)
            } else {
                egui::Color32::from_rgb(255, 59, 48)
            };
            ui.painter().circle_filled(dot_rect.center(), radius, color);
            if resp.clicked() {
                *is_open = false;
            }
            let remaining = ui.available_width();
            ui.add_space((remaining - 50.0).max(0.0) / 2.0);
            ui.label(egui::RichText::new("Export").size(14.0).weak());
        });
        ui.separator();
        ui.add_space(4.0);
    }

    fn render_footer(
        &mut self,
        ui: &mut egui::Ui,
        can_export: bool,
        is_tables_mode: bool,
        is_open: &mut bool,
        outcome: &mut ExportOutcome,
    ) {
        ui.horizontal(|ui| {
            if ui
                .button("Cancel")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *is_open = false;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let btn_text = egui::RichText::new(format!(
                    "{}  Export",
                    egui_phosphor::regular::DOWNLOAD_SIMPLE
                ))
                .color(egui::Color32::WHITE);
                if ui
                    .add_enabled(
                        can_export,
                        egui::Button::new(btn_text).fill(egui::Color32::from_rgb(59, 130, 246)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    if let Some(req) = self.build_request() {
                        *outcome = ExportOutcome::Export(req);
                        *is_open = false;
                    }
                }

                ui.add_space(10.0);
                if is_tables_mode {
                    let count = self.selected_count();
                    ui.label(
                        egui::RichText::new(format!(
                            "{count} table{} to export",
                            if count == 1 { "" } else { "s" }
                        ))
                        .weak()
                        .size(12.0),
                    );
                }
            });
        });
    }
}
