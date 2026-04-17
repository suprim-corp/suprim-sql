/// Context-menu cell action handlers — copy, paste, set value, duplicate, delete.
use eframe::egui;
use suprim_core::db::types::DbValue;
use uuid::Uuid;

use crate::ui::shared::clipboard_formatters;

use super::cell_editor::build_cell_editor;
use super::CellAction;
use super::TableViewerTab;

impl TableViewerTab {
    /// Dispatch a context-menu cell action.
    pub(super) fn handle_cell_action(
        &mut self,
        ui: &egui::Ui,
        action: &CellAction,
        row: usize,
        col: usize,
        _tab_id: Uuid,
    ) {
        let result = match &self.result {
            Some(r) => r,
            None => return,
        };
        let db_val = match result.rows.get(row).and_then(|r| r.get(col)) {
            Some(v) => v.clone(),
            None => return,
        };

        match action {
            CellAction::Copy => ui.ctx().copy_text(db_val.display()),
            CellAction::CopyAsJson => {
                ui.ctx()
                    .copy_text(clipboard_formatters::format_as_json(&db_val));
            }
            CellAction::CopyAsCsv => {
                ui.ctx()
                    .copy_text(clipboard_formatters::format_as_csv(&db_val));
            }
            CellAction::CopyAsSql => {
                ui.ctx()
                    .copy_text(clipboard_formatters::format_as_sql(&db_val));
            }
            CellAction::Paste => {
                // TODO: read from clipboard and update cell
            }
            CellAction::SetNull => {
                self.buffer_set_cell_value(row, col, DbValue::Null);
            }
            CellAction::SetEmpty => {
                self.buffer_set_cell_value(row, col, DbValue::Text(String::new()));
            }
            CellAction::SetDefault => {
                // TODO: resolve the column default from schema and apply
            }
            CellAction::EditValue => {
                if let Some(result) = &self.result {
                    if let Some(editor) = build_cell_editor(result, row, col) {
                        self.cell_editor = Some(editor);
                    }
                }
            }
            CellAction::DuplicateRow => {
                self.buffer_duplicate_row(row);
            }
            CellAction::DeleteRow => {
                self.pending.toggle_delete(row);
            }
        }
    }

    /// Buffer a cell value change into pending changes.
    fn buffer_set_cell_value(&mut self, row: usize, col: usize, value: DbValue) {
        let result = match &self.result {
            Some(r) => r,
            None => return,
        };
        let col_name = match result.columns.get(col) {
            Some(c) => c.name.clone(),
            None => return,
        };
        let original = result
            .rows
            .get(row)
            .and_then(|r| r.get(col))
            .cloned()
            .unwrap_or(DbValue::Null);

        self.pending.edit_cell(row, col, col_name, original, value);
    }

    /// Buffer a duplicate row as a new row insert in pending changes.
    fn buffer_duplicate_row(&mut self, row: usize) {
        let result = match &self.result {
            Some(r) => r,
            None => return,
        };

        let mut values = std::collections::HashMap::new();
        if let Some(row_data) = result.rows.get(row) {
            for (i, col) in result.columns.iter().enumerate() {
                if let Some(val) = row_data.get(i) {
                    values.insert(col.name.clone(), val.clone());
                }
            }
        }

        self.pending.add_row(values);
    }
}
