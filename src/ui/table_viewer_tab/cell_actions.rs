/// Context-menu cell action handlers — copy, paste, set value, duplicate, delete.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::DbValue;
use tokio::sync::mpsc;
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
        tab_id: Uuid,
        cmd_tx: &mpsc::Sender<DbCommand>,
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
                self.set_cell_value(row, col, DbValue::Null, tab_id, cmd_tx);
            }
            CellAction::SetEmpty => {
                self.set_cell_value(row, col, DbValue::Text(String::new()), tab_id, cmd_tx);
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
            CellAction::ExportResults => {
                // TODO: open export dialog
            }
            CellAction::DuplicateRow => {
                self.duplicate_row(row, tab_id, cmd_tx);
            }
            CellAction::DeleteRow => {
                self.delete_row(row, tab_id, cmd_tx);
            }
        }
    }

    /// Set a single cell value via UpdateRow command.
    fn set_cell_value(
        &mut self,
        row: usize,
        col: usize,
        value: DbValue,
        tab_id: Uuid,
        cmd_tx: &mpsc::Sender<DbCommand>,
    ) {
        let result = match &self.result {
            Some(r) => r,
            None => return,
        };
        let col_name = match result.columns.get(col) {
            Some(c) => c.name.clone(),
            None => return,
        };

        let mut pk = std::collections::HashMap::new();
        if let Some(row_data) = result.rows.get(row) {
            for (i, c) in result.columns.iter().enumerate() {
                if let Some(val) = row_data.get(i) {
                    pk.insert(c.name.clone(), val.clone());
                }
            }
        }

        let mut changes = std::collections::HashMap::new();
        changes.insert(col_name, value);

        let schema_table = format!("\"{}\".\"{}\"", self.schema_name, self.table_name);
        let _ = cmd_tx.try_send(DbCommand::UpdateRow {
            conn_id: self.conn_id,
            tab_id,
            table: schema_table,
            pk,
            changes,
        });
        self.load(tab_id, cmd_tx);
    }

    /// Duplicate a row by sending an InsertRow command with all column values.
    fn duplicate_row(&mut self, row: usize, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
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

        let schema_table = format!("\"{}\".\"{}\"", self.schema_name, self.table_name);
        let _ = cmd_tx.try_send(DbCommand::InsertRow {
            conn_id: self.conn_id,
            tab_id,
            table: schema_table,
            values,
        });
        self.load(tab_id, cmd_tx);
    }

    /// Delete a row by sending a DeleteRow command.
    fn delete_row(&mut self, row: usize, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        let result = match &self.result {
            Some(r) => r,
            None => return,
        };

        let mut pk = std::collections::HashMap::new();
        if let Some(row_data) = result.rows.get(row) {
            for (i, col) in result.columns.iter().enumerate() {
                if let Some(val) = row_data.get(i) {
                    pk.insert(col.name.clone(), val.clone());
                }
            }
        }

        let schema_table = format!("\"{}\".\"{}\"", self.schema_name, self.table_name);
        let _ = cmd_tx.try_send(DbCommand::DeleteRow {
            conn_id: self.conn_id,
            tab_id,
            table: schema_table,
            pk,
        });
        self.load(tab_id, cmd_tx);
    }
}
