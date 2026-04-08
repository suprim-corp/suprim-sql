/// Cell editor popup — inline editing for a single cell value with JSON support.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::DbValue;
use tokio::sync::mpsc;
use uuid::Uuid;

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
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a `CellEditor` from a row/col in the current result set.
pub(super) fn build_cell_editor(
    result: &suprim_sql::db::types::QueryResult,
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

        if let Some(editor) = &mut self.cell_editor {
            let mut open = true;
            let title = if editor.is_json {
                format!("Edit JSON: {}", &editor.column_name)
            } else {
                format!("Edit: {}", &editor.column_name)
            };
            let col_name = editor.column_name.clone();
            let is_json = editor.is_json;
            let default_w = if is_json { 520.0 } else { 420.0 };
            let default_h = if is_json { 380.0 } else { 260.0 };
            let min_h = 180.0;

            egui::Window::new(title)
                .open(&mut open)
                .resizable([true, true])
                .default_width(default_w)
                .default_height(default_h)
                .min_height(min_h)
                .pivot(egui::Align2::CENTER_CENTER)
                .default_pos(ui.ctx().content_rect().center())
                .show(ui.ctx(), |ui| {
                    Self::render_editor_header(ui, &col_name, is_json);
                    ui.add_space(4.0);

                    let text_height = (ui.available_height() - 38.0).max(80.0);

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
                    Self::render_editor_buttons(ui, editor, is_json, &mut action);
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

    /// Build and send an UpdateRow command from the current cell editor state.
    fn save_cell_edit(&mut self, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        let editor = match &self.cell_editor {
            Some(e) => e,
            None => return,
        };
        let result = match &self.result {
            Some(r) => r,
            None => return,
        };

        let mut pk = std::collections::HashMap::new();
        if let Some(row_data) = result.rows.get(editor.row) {
            for (i, col) in result.columns.iter().enumerate() {
                if let Some(val) = row_data.get(i) {
                    pk.insert(col.name.clone(), val.clone());
                }
            }
        }

        let mut changes = std::collections::HashMap::new();
        changes.insert(
            editor.column_name.clone(),
            DbValue::Text(editor.edit_value.clone()),
        );

        let schema_table = format!("\"{}\".\"{}\"", self.schema_name, self.table_name);

        let _ = cmd_tx.try_send(DbCommand::UpdateRow {
            conn_id: self.conn_id,
            tab_id,
            table: schema_table,
            pk,
            changes,
        });

        self.cell_editor = None;
        self.load(tab_id, cmd_tx);
    }
}
