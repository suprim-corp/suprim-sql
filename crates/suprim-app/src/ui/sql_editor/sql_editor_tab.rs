/// SQL Editor tab — interactive query execution and result display.
use eframe::egui;
use suprim_core::db::commands::DbCommand;
use suprim_core::db::types::QueryResult;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::sql_autocomplete::AutocompleteState;
use crate::ui::shared::clipboard_formatters;
use crate::ui::shared::result_grid::{render_result_grid, CellAction};
use crate::ui::table_viewer_tab::pending_changes::PendingChanges;

pub struct SqlEditorTab {
    pub conn_id: Option<Uuid>,
    /// Target database for query execution (cross-database support).
    pub database: Option<String>,
    /// Available databases for the connection (drives the database picker).
    pub databases: Vec<String>,
    pub(crate) sql_text: String,
    result: Option<QueryResult>,
    /// Pre-computed display strings for each cell — avoids per-frame allocations.
    display_cache: Vec<Vec<String>>,
    pub is_running: bool,
    /// Set when the last query execution failed (shows error icon).
    last_run_failed: bool,
    /// Currently selected data cell (row_idx, col_idx) for highlight + copy.
    selected_cell: Option<(usize, usize)>,
    /// Currently selected entire row (row_idx) — click on row number to select.
    selected_row: Option<usize>,
    /// SQL keyword autocomplete state.
    pub(crate) autocomplete: AutocompleteState,
}

impl SqlEditorTab {
    pub fn new(conn_id: Option<Uuid>, database: Option<String>, databases: Vec<String>) -> Self {
        Self {
            conn_id,
            database,
            databases,
            sql_text: String::new(),
            result: None,
            display_cache: Vec::new(),
            is_running: false,
            last_run_failed: false,
            selected_cell: None,
            selected_row: None,
            autocomplete: AutocompleteState::new(),
        }
    }

    pub fn set_result(&mut self, result: QueryResult, cache: Vec<Vec<String>>) {
        self.result = Some(result);
        self.display_cache = cache;
        self.is_running = false;
        self.last_run_failed = false;
    }

    /// Returns the current SQL text in the editor.
    pub fn sql_text(&self) -> &str {
        &self.sql_text
    }

    /// Replace the editor's SQL text (used by history panel "load into editor").
    pub fn set_sql_text(&mut self, sql: &str) {
        self.sql_text = sql.to_string();
    }

    /// Format/prettify the SQL text using sqlformat.
    fn format_sql(&mut self) {
        let options = sqlformat::FormatOptions {
            indent: sqlformat::Indent::Spaces(2),
            uppercase: Some(true),
            lines_between_queries: 2,
            ..Default::default()
        };
        self.sql_text = sqlformat::format(&self.sql_text, &sqlformat::QueryParams::None, &options);
    }

    /// Called when the query execution failed.
    pub fn on_error(&mut self) {
        self.is_running = false;
        self.last_run_failed = true;
    }

    /// Handle a cell action from the context menu (SQL editor only supports copy actions).
    fn handle_cell_action(&self, ui: &egui::Ui, action: &CellAction, row: usize, col: usize) {
        let result = match &self.result {
            Some(r) => r,
            None => return,
        };
        let db_val = match result.rows.get(row).and_then(|r| r.get(col)) {
            Some(v) => v,
            None => return,
        };
        match action {
            CellAction::Copy => ui.ctx().copy_text(db_val.display()),
            CellAction::CopyAsJson => ui
                .ctx()
                .copy_text(clipboard_formatters::format_as_json(db_val)),
            CellAction::CopyAsCsv => ui
                .ctx()
                .copy_text(clipboard_formatters::format_as_csv(db_val)),
            CellAction::CopyAsSql => ui
                .ctx()
                .copy_text(clipboard_formatters::format_as_sql(db_val)),
            // Other actions are not supported in the SQL editor (read-only results)
            _ => {}
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        // ⌘+Enter (or Ctrl+Enter) keyboard shortcut to run query.
        let can_run = self.conn_id.is_some() && !self.is_running;
        let cmd_enter = ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
        if can_run && cmd_enter {
            if let Some(conn_id) = self.conn_id {
                let _ = cmd_tx.try_send(DbCommand::Execute {
                    conn_id,
                    tab_id,
                    sql: self.sql_text.clone(),
                    database: self.database.clone(),
                });
                self.is_running = true;
                self.last_run_failed = false;
            }
        }

        // Shift+⌘+F (or Ctrl+Shift+F) keyboard shortcut to format SQL.
        let shift_cmd_f =
            ui.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.command && i.modifiers.shift);
        if shift_cmd_f && !self.sql_text.trim().is_empty() {
            self.format_sql();
        }

        ui.vertical(|ui| {
            // Toolbar row
            ui.horizontal(|ui| {
                let run_btn = egui::Button::new(egui::RichText::new(format!(
                    "{} Run",
                    egui_phosphor::regular::PLAY
                )));
                let can_run = self.conn_id.is_some() && !self.is_running;
                let run_resp = ui
                    .add_enabled(can_run, run_btn)
                    .on_hover_cursor(egui::CursorIcon::PointingHand);

                // Shortcut hint rendered with Phosphor icons.
                let weak = ui.visuals().weak_text_color();
                if cfg!(target_os = "macos") {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}{}",
                            egui_phosphor::regular::COMMAND,
                            egui_phosphor::regular::KEY_RETURN,
                        ))
                        .color(weak),
                    );
                } else {
                    ui.label(egui::RichText::new("Ctrl").color(weak).small());
                    ui.label(egui::RichText::new(egui_phosphor::regular::KEY_RETURN).color(weak));
                }
                if run_resp.clicked() {
                    if let Some(conn_id) = self.conn_id {
                        let _ = cmd_tx.try_send(DbCommand::Execute {
                            conn_id,
                            tab_id,
                            sql: self.sql_text.clone(),
                            database: self.database.clone(),
                        });
                        self.is_running = true;
                        self.last_run_failed = false;
                    }
                }

                if self.is_running {
                    ui.spinner();
                } else if self.last_run_failed {
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::X_CIRCLE)
                            .color(egui::Color32::from_rgb(220, 60, 60))
                            .size(18.0),
                    );
                }

                // Format SQL button
                ui.separator();
                let fmt_enabled = !self.sql_text.trim().is_empty();
                let fmt_btn = egui::Button::new(egui::RichText::new(format!(
                    "{} Format",
                    egui_phosphor::regular::MAGIC_WAND
                )));
                if ui
                    .add_enabled(fmt_enabled, fmt_btn)
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Format SQL (⇧⌘F)")
                    .clicked()
                {
                    self.format_sql();
                }

                // Database picker — lets user choose which database to execute against.
                if !self.databases.is_empty() {
                    ui.separator();
                    let db_icon = egui_phosphor::regular::DATABASE;
                    let current_label = self.database.as_deref().unwrap_or("(default)");
                    let combo_resp = egui::ComboBox::from_id_salt("sql_db_picker")
                        .selected_text(format!("{db_icon} {current_label}"))
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            for db_name in &self.databases {
                                let is_selected =
                                    self.database.as_deref() == Some(db_name.as_str());
                                if ui
                                    .selectable_label(is_selected, db_name)
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    self.database = Some(db_name.clone());
                                }
                            }
                        });
                    combo_resp
                        .response
                        .on_hover_cursor(egui::CursorIcon::PointingHand);
                }

                if self.conn_id.is_none() {
                    ui.label(
                        egui::RichText::new("No connection selected").color(egui::Color32::YELLOW),
                    );
                }
            });

            ui.separator();

            // SQL text editor (top half) — syntax highlighting + autocomplete
            let available = ui.available_height();
            let editor_height = (available * 0.4).max(80.0);
            self.render_editor_area(ui, editor_height);

            ui.separator();

            // Results grid (bottom half)
            if let Some(result) = &self.result {
                let no_pending = PendingChanges::new();
                let grid_out = render_result_grid(
                    ui,
                    result,
                    &self.display_cache,
                    &mut self.selected_cell,
                    &mut self.selected_row,
                    &no_pending,
                );
                // Handle context-menu actions (read-only for SQL editor — only copy actions)
                if let Some((action, row, col)) = grid_out.action {
                    self.handle_cell_action(ui, &action, row, col);
                }
            } else {
                let weak = ui.visuals().weak_text_color();
                ui.label(egui::RichText::new("Run a query to see results").color(weak));
            }
        });
    }
}
