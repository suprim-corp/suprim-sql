/// Table Viewer tab — browse table data with pagination, filtering, cell editing.
mod cell_actions;
mod cell_editor;
mod cell_editor_widgets;
mod filter_bar;
mod pagination_bar;

use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::QueryResult;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::result_grid::{render_result_grid, CellAction};
use cell_editor::{build_cell_editor, CellEditor};

// ── TableViewerTab ────────────────────────────────────────────────────────────

pub struct TableViewerTab {
    pub conn_id: Uuid,
    database: String,
    schema_name: String,
    pub table_name: String,
    result: Option<QueryResult>,
    /// Pre-computed display strings for each cell — avoids per-frame allocations.
    display_cache: Vec<Vec<String>>,
    page: usize,
    page_size: usize,
    /// Total row count from the DB (for pagination display).
    total_count: Option<u64>,
    pub is_loading: bool,
    /// True until the first load is dispatched (auto-load on open).
    needs_initial_load: bool,
    where_clause: String,
    order_clause: String,
    /// Currently selected data cell (row_idx, col_idx) for highlight + copy.
    selected_cell: Option<(usize, usize)>,
    /// Popup cell editor opened by double-click.
    cell_editor: Option<CellEditor>,
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
            cell_editor: None,
        }
    }

    pub fn set_result(&mut self, result: QueryResult, cache: Vec<Vec<String>>) {
        self.total_count = result.total_count;
        self.result = Some(result);
        self.display_cache = cache;
        self.is_loading = false;
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

    pub fn show(&mut self, ui: &mut egui::Ui, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        // Auto-load data on first render.
        if self.needs_initial_load {
            self.needs_initial_load = false;
            self.load(tab_id, cmd_tx);
        }

        // Derive colors from the current theme.
        let vis = ui.visuals().clone();
        let bar_bg = vis.faint_bg_color;
        let bar_stroke_color = vis.widgets.noninteractive.bg_stroke.color;
        let hint_color = vis.weak_text_color();

        ui.vertical(|ui| {
            self.render_filter_bar(ui, tab_id, cmd_tx, bar_bg, bar_stroke_color, hint_color);
            self.render_pagination_bar(ui, tab_id, cmd_tx, hint_color);

            // ── Result grid + context menu ──
            let mut pending_action: Option<(CellAction, usize, usize)> = None;
            let mut pending_double_click: Option<(usize, usize)> = None;

            if let Some(result) = &self.result {
                let grid_out =
                    render_result_grid(ui, result, &self.display_cache, &mut self.selected_cell);
                pending_action = grid_out.action;
                pending_double_click = grid_out.double_clicked;
            } else if self.is_loading {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            }

            // Handle context-menu actions (borrow of self.result is now dropped).
            if let Some((action, row, col)) = pending_action {
                self.handle_cell_action(ui, &action, row, col, tab_id, cmd_tx);
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
        });
    }
}
