/// Table Viewer tab — browse table data with pagination, filtering, cell editing.
mod cell_actions;
mod cell_editor;
mod cell_editor_widgets;
mod filter_bar;
mod new_row_editor;
mod pagination_bar;
pub(crate) mod pending_changes;
pub(crate) mod sql_preview;
mod toolbar;

use eframe::egui;
use suprim_core::db::commands::DbCommand;
use suprim_core::db::types::{DbValue, QueryResult};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::ui::shared::result_grid::{render_result_grid, CellAction};
use crate::ui::sql_editor::sql_autocomplete::AutocompleteState;
use cell_editor::{build_cell_editor, CellEditor};
use new_row_editor::NewRowEditor;
use pending_changes::PendingChanges;

// ── TableViewerTab ────────────────────────────────────────────────────────────

pub struct TableViewerTab {
    pub conn_id: Uuid,
    pub database: String,
    pub schema_name: String,
    pub table_name: String,
    result: Option<QueryResult>,
    /// Pre-computed display strings for each cell — avoids per-frame allocations.
    display_cache: Vec<Vec<String>>,
    pub(crate) page: usize,
    pub(crate) page_size: usize,
    /// Total row count from the DB (for pagination display).
    total_count: Option<u64>,
    pub is_loading: bool,
    /// True until the first load is dispatched (auto-load on open).
    needs_initial_load: bool,
    pub(crate) where_clause: String,
    pub(crate) order_clause: String,
    /// Currently selected data cell (row_idx, col_idx) for highlight + copy.
    selected_cell: Option<(usize, usize)>,
    /// Currently selected entire row (row_idx) — click on row number to select.
    selected_row: Option<usize>,
    /// Popup cell editor opened by double-click.
    cell_editor: Option<CellEditor>,
    /// New row editor popup.
    new_row_editor: Option<NewRowEditor>,
    /// Deferred actions from toolbar (processed after filter_bar borrow ends).
    pending_toolbar_delete: bool,
    pending_undo: bool,
    pending_execute: bool,
    /// Batch pending changes — delete/edit/add buffered until Execute.
    pub(super) pending: PendingChanges,
    /// Autocomplete state for WHERE filter input.
    where_autocomplete: AutocompleteState,
    /// Autocomplete state for ORDER BY filter input.
    order_autocomplete: AutocompleteState,
    /// Set by event_handler when RowMutated arrives — triggers reload on next frame.
    pub needs_reload_after_mutation: bool,
}

impl TableViewerTab {
    pub fn new(conn_id: Uuid, database: String, schema_name: String, table_name: String) -> Self {
        Self {
            conn_id,
            database,
            schema_name,
            table_name,
            result: None,
            display_cache: Vec::new(),
            page: 0,
            page_size: 100,
            total_count: None,
            is_loading: false,
            needs_initial_load: true,
            where_clause: String::new(),
            order_clause: String::new(),
            selected_cell: None,
            selected_row: None,
            cell_editor: None,
            new_row_editor: None,
            pending_toolbar_delete: false,
            pending_undo: false,
            pending_execute: false,
            pending: PendingChanges::new(),
            where_autocomplete: AutocompleteState::new(),
            order_autocomplete: AutocompleteState::new(),
            needs_reload_after_mutation: false,
        }
    }

    pub fn where_clause_text(&self) -> &str {
        &self.where_clause
    }

    pub fn order_clause_text(&self) -> &str {
        &self.order_clause
    }

    pub fn set_result(&mut self, result: QueryResult, cache: Vec<Vec<String>>) {
        self.total_count = result.total_count;
        self.result = Some(result);
        self.display_cache = cache;
        self.is_loading = false;
        // Clear pending changes when fresh data arrives — prevents stale row indices.
        self.pending.clear();
    }

    pub(crate) fn load(&mut self, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        let where_opt = {
            let w = self.where_clause.trim().to_string();
            if w.is_empty() {
                None
            } else {
                Some(w)
            }
        };
        let order_opt = {
            let o = self.order_clause.trim().to_string();
            if o.is_empty() {
                None
            } else {
                Some(o)
            }
        };
        let _ = cmd_tx.try_send(DbCommand::LoadTableData {
            conn_id: self.conn_id,
            tab_id,
            database: Some(self.database.clone()),
            schema: Some(self.schema_name.clone()),
            table: self.table_name.clone(),
            page: self.page as u32,
            page_size: self.page_size as u32,
            where_clause: where_opt,
            order_clause: order_opt,
        });
        self.is_loading = true;
    }

    /// Execute all pending changes — send DELETE/UPDATE/INSERT commands to DB.
    fn execute_pending_changes(&mut self, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        if !self.pending.has_changes() {
            return;
        }
        let result = match &self.result {
            Some(r) => r,
            None => return,
        };
        let schema_table = format!("{}.{}", self.schema_name, self.table_name);

        // 1. Deletes
        for &row_idx in &self.pending.deleted_rows {
            let mut pk = std::collections::HashMap::new();
            if let Some(row_data) = result.rows.get(row_idx) {
                for (i, col) in result.columns.iter().enumerate() {
                    if let Some(val) = row_data.get(i) {
                        pk.insert(col.name.clone(), val.clone());
                    }
                }
            }
            let _ = cmd_tx.try_send(DbCommand::DeleteRow {
                conn_id: self.conn_id,
                tab_id,
                table: schema_table.clone(),
                pk,
            });
        }

        // 2. Edits — group by row to send one UpdateRow per row
        let mut edits_by_row: std::collections::HashMap<
            usize,
            std::collections::HashMap<String, DbValue>,
        > = std::collections::HashMap::new();
        for (&(row_idx, _col_idx), edited) in &self.pending.edited_cells {
            // Skip edits on rows that are also deleted
            if self.pending.deleted_rows.contains(&row_idx) {
                continue;
            }
            edits_by_row
                .entry(row_idx)
                .or_default()
                .insert(edited.column_name.clone(), edited.new_value.clone());
        }
        for (row_idx, changes) in edits_by_row {
            let mut pk = std::collections::HashMap::new();
            if let Some(row_data) = result.rows.get(row_idx) {
                for (i, col) in result.columns.iter().enumerate() {
                    if let Some(val) = row_data.get(i) {
                        pk.insert(col.name.clone(), val.clone());
                    }
                }
            }
            let _ = cmd_tx.try_send(DbCommand::UpdateRow {
                conn_id: self.conn_id,
                tab_id,
                table: schema_table.clone(),
                pk,
                changes,
            });
        }

        // 3. Inserts
        for new_row in &self.pending.new_rows {
            let _ = cmd_tx.try_send(DbCommand::InsertRow {
                conn_id: self.conn_id,
                tab_id,
                table: schema_table.clone(),
                values: new_row.values.clone(),
            });
        }

        self.pending.clear();
        self.is_loading = true;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        // Auto-load data on first render.
        if self.needs_initial_load {
            self.needs_initial_load = false;
            self.load(tab_id, cmd_tx);
        }

        // Reload after a mutation (delete/update/insert) confirmed by the worker.
        if self.needs_reload_after_mutation {
            self.needs_reload_after_mutation = false;
            self.load(tab_id, cmd_tx);
        }

        // Derive colors from the current theme.
        let vis = ui.visuals().clone();
        let bar_bg = vis.faint_bg_color;
        let bar_stroke_color = vis.widgets.noninteractive.bg_stroke.color;
        let hint_color = vis.weak_text_color();

        ui.vertical(|ui| {
            self.render_filter_bar(ui, tab_id, cmd_tx, bar_bg, bar_stroke_color, hint_color);

            // Handle deferred toolbar actions (after filter_bar borrow ends)
            if self.pending_toolbar_delete {
                self.pending_toolbar_delete = false;
                let row_to_delete = self
                    .selected_row
                    .or_else(|| self.selected_cell.map(|(r, _)| r));
                if let Some(row) = row_to_delete {
                    self.pending.toggle_delete(row);
                }
            }

            if self.pending_undo {
                self.pending_undo = false;
                self.pending.undo();
            }

            if self.pending_execute {
                self.pending_execute = false;
                self.execute_pending_changes(tab_id, cmd_tx);
            }

            self.render_pagination_bar(ui, tab_id, cmd_tx, hint_color);

            // ── Result grid + context menu ──
            let mut pending_action: Option<(CellAction, usize, usize)> = None;
            let mut pending_double_click: Option<(usize, usize)> = None;

            if let Some(result) = &self.result {
                let grid_out = render_result_grid(
                    ui,
                    result,
                    &self.display_cache,
                    &mut self.selected_cell,
                    &mut self.selected_row,
                    &self.pending,
                );
                pending_action = grid_out.action;
                pending_double_click = grid_out.double_clicked;
            } else if self.is_loading {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            }

            // Handle context-menu actions (borrow of self.result is now dropped).
            if let Some((action, row, col)) = pending_action {
                self.handle_cell_action(ui, &action, row, col, tab_id);
            }

            // Double-click → open cell editor popup
            if let Some((row, col)) = pending_double_click {
                if let Some(result) = &self.result {
                    if let Some(editor) = build_cell_editor(result, row, col) {
                        self.cell_editor = Some(editor);
                    }
                }
            }

            // ── Cell editor popup ──
            self.render_cell_editor_popup(ui, tab_id, cmd_tx);

            // ── New row editor popup ──
            self.render_new_row_editor(ui.ctx(), tab_id);
        });
    }
}
