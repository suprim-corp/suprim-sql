/// SQL Editor tab — interactive query execution and result display.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::QueryResult;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::sql_autocomplete::{self, AutocompleteState};
use crate::ui::shared::clipboard_formatters;
use crate::ui::shared::result_grid::{render_result_grid, CellAction};
use crate::ui::table_viewer_tab::pending_changes::PendingChanges;

pub struct SqlEditorTab {
    pub conn_id: Option<Uuid>,
    /// Target database for query execution (cross-database support).
    pub database: Option<String>,
    /// Available databases for the connection (drives the database picker).
    pub databases: Vec<String>,
    sql_text: String,
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
    autocomplete: AutocompleteState,
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

            // SQL text editor (top half)
            let available = ui.available_height();
            let editor_height = (available * 0.4).max(80.0);

            // Collect input events BEFORE rendering the editor (needed for auto-pair).
            let input_events: Vec<egui::Event> = ui.input(|i| i.events.clone());

            // Phase 0: Consume autocomplete navigation keys BEFORE TextEdit
            // so Enter/Tab/Arrow don't reach the editor.
            sql_autocomplete::consume_autocomplete_keys(ui, &mut self.autocomplete);

            let text_edit_id = egui::Id::new("sql_editor_textedit");
            let dark_mode = ui.visuals().dark_mode;
            let mono_font = egui::FontId::monospace(14.0);
            let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap_width: f32| {
                let job = super::sql_highlighter::sql_layout_job(
                    text.as_str(),
                    mono_font.clone(),
                    dark_mode,
                );
                ui.fonts_mut(|f| f.layout_job(job))
            };
            let scroll_out = egui::ScrollArea::vertical()
                .id_salt("sql_editor_scroll")
                .max_height(editor_height)
                .show(ui, |ui| {
                    egui::TextEdit::multiline(&mut self.sql_text)
                        .id(text_edit_id)
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(10)
                        .desired_width(f32::INFINITY)
                        .hint_text("SELECT …")
                        .layouter(&mut layouter)
                        .show(ui)
                });

            let te_output = scroll_out.inner;

            // --- Auto-pair: insert matching close bracket ---
            // CCursor.index is a *character* offset (not byte offset).
            let cursor_char_pos = te_output.cursor_range.map(|cr| cr.primary.index);

            if te_output.response.changed() {
                if let Some(pos) = cursor_char_pos {
                    if sql_autocomplete::handle_auto_pair(
                        &mut self.sql_text,
                        Some(pos),
                        &input_events,
                    ) {
                        // Move cursor back between the pair (re-set cursor position).
                        if let Some(mut state) =
                            egui::TextEdit::load_state(ui.ctx(), te_output.response.id)
                        {
                            let cc = egui::text::CCursor::new(pos);
                            state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                            state.store(ui.ctx(), te_output.response.id);
                        }
                    }
                }
            }

            // --- Keyword autocomplete ---
            let has_focus = te_output.response.has_focus();
            if has_focus {
                if let Some(pos) = cursor_char_pos {
                    sql_autocomplete::update_autocomplete(
                        &mut self.autocomplete,
                        &self.sql_text,
                        pos,
                        &[], // SQL editor: no extra column suggestions
                    );
                }

                // Compute cursor screen position from galley for popup anchoring.
                let cursor_screen_pos = cursor_char_pos.map(|char_pos| {
                    let ccursor = egui::text::CCursor::new(char_pos);
                    let galley_offset = te_output.galley.pos_from_cursor(ccursor);
                    // galley_offset is relative to galley_pos; shift to screen coords.
                    // Place popup one line below the cursor.
                    let line_height = te_output
                        .galley
                        .rows
                        .first()
                        .map(|r| r.rect().height())
                        .unwrap_or(16.0);
                    egui::pos2(
                        te_output.galley_pos.x + galley_offset.min.x,
                        te_output.galley_pos.y + galley_offset.min.y + line_height,
                    )
                });

                if let Some(accepted) = sql_autocomplete::show_autocomplete_popup(
                    ui,
                    &mut self.autocomplete,
                    text_edit_id,
                    cursor_screen_pos,
                ) {
                    let new_cursor = sql_autocomplete::apply_suggestion(
                        &mut self.sql_text,
                        accepted.prefix_char_start,
                        accepted.prefix_char_len,
                        &accepted.replacement,
                    );
                    // Update cursor position after replacement.
                    if let Some(mut state) =
                        egui::TextEdit::load_state(ui.ctx(), te_output.response.id)
                    {
                        let cc = egui::text::CCursor::new(new_cursor);
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                        state.store(ui.ctx(), te_output.response.id);
                    }
                    // Re-focus the editor.
                    te_output.response.request_focus();
                }
            } else {
                self.autocomplete.close();
            }

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
