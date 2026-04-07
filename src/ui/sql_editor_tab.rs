/// SQL Editor tab — interactive query execution and result display.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::QueryResult;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::clipboard_formatters;
use super::result_grid::{render_result_grid, CellAction};

pub struct SqlEditorTab {
    pub conn_id: Option<Uuid>,
    sql_text: String,
    result: Option<QueryResult>,
    /// Pre-computed display strings for each cell — avoids per-frame allocations.
    display_cache: Vec<Vec<String>>,
    pub is_running: bool,
    /// Currently selected data cell (row_idx, col_idx) for highlight + copy.
    selected_cell: Option<(usize, usize)>,
}

impl SqlEditorTab {
    pub fn new(conn_id: Option<Uuid>) -> Self {
        Self {
            conn_id,
            sql_text: String::new(),
            result: None,
            display_cache: Vec::new(),
            is_running: false,
            selected_cell: None,
        }
    }

    pub fn set_result(&mut self, result: QueryResult, cache: Vec<Vec<String>>) {
        self.result = Some(result);
        self.display_cache = cache;
        self.is_running = false;
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
        ui.vertical(|ui| {
            // Toolbar row
            ui.horizontal(|ui| {
                let run_btn = egui::Button::new(egui::RichText::new(format!(
                    "{} Run",
                    egui_phosphor::regular::PLAY
                )));
                let can_run = self.conn_id.is_some() && !self.is_running;
                if ui.add_enabled(can_run, run_btn).clicked() {
                    if let Some(conn_id) = self.conn_id {
                        let _ = cmd_tx.try_send(DbCommand::Execute {
                            conn_id,
                            tab_id,
                            sql: self.sql_text.clone(),
                        });
                        self.is_running = true;
                    }
                }

                if self.is_running {
                    ui.spinner();
                }

                if self.conn_id.is_none() {
                    ui.label(
                        egui::RichText::new("No connection selected").color(egui::Color32::YELLOW),
                    );
                }
            });

            ui.separator();

            // SQL text editor (top half)
            let available = ui.available_height();
            let editor_height = (available * 0.4).max(80.0);
            egui::ScrollArea::vertical()
                .id_salt("sql_editor_scroll")
                .max_height(editor_height)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.sql_text)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(10)
                            .desired_width(f32::INFINITY)
                            .hint_text("SELECT …"),
                    );
                });

            ui.separator();

            // Results grid (bottom half)
            if let Some(result) = &self.result {
                let grid_out =
                    render_result_grid(ui, result, &self.display_cache, &mut self.selected_cell);
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
