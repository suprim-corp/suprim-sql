/// Cell editor popup — inline editing for a single cell value with JSON support.
use eframe::egui;
use suprim_core::db::commands::DbCommand;
use suprim_core::db::types::DbValue;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::sql_preview;
use super::TableViewerTab;

// ── Types ─────────────────────────────────────────────────────────────────────

pub(super) enum CellEditorAction {
    None,
    Save,
    Close,
}

pub(super) struct CellEditor {
    pub row: usize,
    #[allow(dead_code)]
    pub col: usize,
    pub column_name: String,
    pub original_value: String,
    pub edit_value: String,
    pub is_json: bool,
    pub json_error: Option<String>,
    /// Whether to show the SQL preview panel below the editor.
    pub show_sql_preview: bool,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a `CellEditor` from a row/col in the current result set.
pub(super) fn build_cell_editor(
    result: &suprim_core::db::types::QueryResult,
    row: usize,
    col: usize,
) -> Option<CellEditor> {
    let col_meta = result.columns.get(col)?;
    let db_val = result.rows.get(row).and_then(|r| r.get(col));
    let (raw, is_json) = match db_val {
        Some(DbValue::Json(v)) => {
            let pretty = serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string());
            (pretty, true)
        }
        Some(v) => {
            let s = v.display();
            let looks_json = s.starts_with('{') || s.starts_with('[');
            if looks_json {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&s) {
                    let pretty = serde_json::to_string_pretty(&parsed).unwrap_or(s.clone());
                    (pretty, true)
                } else {
                    (s, false)
                }
            } else {
                (s, false)
            }
        }
        None => (String::new(), false),
    };
    Some(CellEditor {
        row,
        col,
        column_name: col_meta.name.clone(),
        original_value: raw.clone(),
        edit_value: raw,
        is_json,
        json_error: None,
        show_sql_preview: false,
    })
}

// ── Impl on TableViewerTab ────────────────────────────────────────────────────

impl TableViewerTab {
    /// Render the cell-editor popup when active.
    pub(super) fn render_cell_editor_popup(
        &mut self,
        ui: &mut egui::Ui,
        tab_id: Uuid,
        cmd_tx: &mpsc::Sender<DbCommand>,
    ) {
        let mut action = CellEditorAction::None;

        // Collect schema/table outside the borrow of cell_editor
        let schema = self.schema_name.clone();
        let table = self.table_name.clone();

        if let Some(editor) = &mut self.cell_editor {
            let mut open = true;
            let title = if editor.is_json {
                format!("Edit JSON: {}", &editor.column_name)
            } else {
                format!("Edit: {}", &editor.column_name)
            };
            let col_name = editor.column_name.clone();
            let is_json = editor.is_json;
            let has_preview = editor.show_sql_preview;
            let default_w = if is_json { 520.0 } else { 420.0 };
            let default_h = if is_json { 380.0 } else { 260.0 };
            let min_h = 180.0;

            egui::Window::new(title)
                .open(&mut open)
                .resizable([true, true])
                .default_width(default_w)
                .default_height(default_h + if has_preview { 120.0 } else { 0.0 })
                .min_height(min_h)
                .pivot(egui::Align2::CENTER_CENTER)
                .default_pos(ui.ctx().content_rect().center())
                .show(ui.ctx(), |ui| {
                    Self::render_editor_header(ui, &col_name, is_json);
                    ui.add_space(4.0);

                    let preview_h = if editor.show_sql_preview { 130.0 } else { 0.0 };
                    let text_height = (ui.available_height() - 38.0 - preview_h).max(80.0);

                    if is_json {
                        Self::render_json_editor(ui, &mut editor.edit_value, text_height);
                    } else {
                        Self::render_plain_editor(ui, &mut editor.edit_value, text_height);
                    }

                    // JSON validation error message
                    if let Some(err) = &editor.json_error {
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(err)
                                .small()
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                    }

                    ui.add_space(4.0);

                    // SQL Preview toggle + buttons
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(editor.show_sql_preview, "SQL Preview")
                            .clicked()
                        {
                            editor.show_sql_preview = !editor.show_sql_preview;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            Self::render_editor_buttons_inline(ui, editor, is_json, &mut action);
                        });
                    });

                    // SQL Preview panel
                    if editor.show_sql_preview {
                        let changed = editor.edit_value != editor.original_value;
                        if changed {
                            // Build preview SQL
                            let new_val = DbValue::Text(editor.edit_value.clone());
                            let mut pk = std::collections::HashMap::new();
                            if let Some(result) = &self.result {
                                if let Some(row_data) = result.rows.get(editor.row) {
                                    pk = sql_preview::build_pk_from_row(&result.columns, row_data);
                                }
                            }
                            let preview = sql_preview::preview_update_sql(
                                &schema,
                                &table,
                                &editor.column_name,
                                &new_val,
                                &pk,
                            );
                            ui.add_space(4.0);
                            egui::ScrollArea::vertical()
                                .id_salt("cell_sql_preview")
                                .max_height(100.0)
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut preview.as_str())
                                            .code_editor()
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                        } else {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("No changes to preview")
                                    .weak()
                                    .italics(),
                            );
                        }
                    }
                });
            if !open {
                action = CellEditorAction::Close;
            }
        }

        match action {
            CellEditorAction::Save => self.save_cell_edit(tab_id, cmd_tx),
            CellEditorAction::Close => self.cell_editor = None,
            CellEditorAction::None => {}
        }
    }

    /// Buffer the cell edit into pending changes (does NOT send to DB).
    fn save_cell_edit(&mut self, _tab_id: Uuid, _cmd_tx: &mpsc::Sender<DbCommand>) {
        let editor = match &self.cell_editor {
            Some(e) => e,
            None => return,
        };
        let result = match &self.result {
            Some(r) => r,
            None => return,
        };

        // Find column index
        let col_idx = match result
            .columns
            .iter()
            .position(|c| c.name == editor.column_name)
        {
            Some(idx) => idx,
            None => return,
        };

        // Get original value from the result
        let original_value = result
            .rows
            .get(editor.row)
            .and_then(|row| row.get(col_idx))
            .cloned()
            .unwrap_or(DbValue::Null);

        let new_value = DbValue::Text(editor.edit_value.clone());

        // Buffer into pending changes
        self.pending.edit_cell(
            editor.row,
            col_idx,
            editor.column_name.clone(),
            original_value,
            new_value,
        );

        self.cell_editor = None;
    }
}
