/// Shared result-grid renderer — used by both SqlEditorTab and TableViewerTab.
/// Uses egui_extras::TableBuilder for responsive column widths and built-in virtual scrolling.
///
/// Context-menu rendering is in `result_grid_context_menu.rs`.
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::cell::RefCell;
use std::rc::Rc;
use suprim_sql::db::types::QueryResult;

use super::result_grid_context_menu::render_cell_context_menu;
use crate::ui::table_viewer_tab::pending_changes::PendingChanges;

// ── Cell context-menu actions ─────────────────────────────────────────────────

/// Actions returned by the result grid when the user interacts via context menu.
#[derive(Debug, Clone)]
pub enum CellAction {
    /// Copy the raw cell value to clipboard (Cmd+C).
    Copy,
    /// Copy the cell value formatted as JSON.
    CopyAsJson,
    /// Copy the cell value as a CSV fragment.
    CopyAsCsv,
    /// Copy the cell value as a SQL literal.
    CopyAsSql,
    /// Paste clipboard contents into the cell (Cmd+V) — only meaningful in editable tabs.
    Paste,
    /// Set the cell value to NULL.
    SetNull,
    /// Set the cell value to an empty string.
    SetEmpty,
    /// Set the cell value to the default column value.
    SetDefault,
    /// Export all results (opens export dialog).
    ExportResults,
    /// Duplicate the selected row (Cmd+D).
    DuplicateRow,
    /// Delete the selected row (Backspace/Delete key).
    DeleteRow,
    /// Open the cell editor (double-click equivalent).
    EditValue,
}

/// Output of `render_result_grid` — carries both double-click and context-menu info.
pub struct GridOutput {
    /// Cell that was double-clicked (row, col).
    pub double_clicked: Option<(usize, usize)>,
    /// Context-menu action with the target cell (row, col).
    pub action: Option<(CellAction, usize, usize)>,
}

/// Pre-compute display strings for all cells once (avoids per-frame allocations).
pub fn build_display_cache(result: &QueryResult) -> Vec<Vec<String>> {
    result
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut cached = Vec::with_capacity(row.len() + 1);
            // First entry is the row number (1-indexed).
            cached.push(format!("{}", i + 1));
            for v in row {
                cached.push(v.display());
            }
            cached
        })
        .collect()
}

/// Minimum column width for data columns (px).
const MIN_COL_WIDTH: f32 = 80.0;
/// Row-number column width (px).
const ROW_NUM_WIDTH: f32 = 44.0;
/// Height of each row (px).
const ROW_HEIGHT: f32 = 22.0;

/// Render the result grid and return a `GridOutput` with double-click and context-menu actions.
pub fn render_result_grid(
    ui: &mut egui::Ui,
    result: &QueryResult,
    display_cache: &[Vec<String>],
    selected_cell: &mut Option<(usize, usize)>,
    selected_row: &mut Option<usize>,
    pending: &PendingChanges,
) -> GridOutput {
    let mut output = GridOutput {
        double_clicked: None,
        action: None,
    };
    let num_rows = result.rows.len();
    let num_cols = result.columns.len();
    let weak = ui.visuals().weak_text_color();
    let selection_fill = ui.visuals().selection.bg_fill;
    let delete_fill = egui::Color32::from_rgba_premultiplied(180, 40, 40, 20);
    let edit_fill = egui::Color32::from_rgba_premultiplied(220, 180, 50, 25);
    let delete_text = if ui.visuals().dark_mode {
        egui::Color32::from_rgb(240, 130, 130)
    } else {
        egui::Color32::from_rgb(180, 60, 60)
    };

    // Handle Cmd+C / Ctrl+C to copy selected cell raw value
    if let Some((row, col)) = *selected_cell {
        let copy_requested = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
        if copy_requested {
            if let Some(db_val) = result.rows.get(row).and_then(|r| r.get(col)) {
                let raw = db_val.display();
                ui.ctx().copy_text(raw);
            }
        }
    }

    // Shared cell for collecting context-menu actions from inside closures.
    let pending_action: Rc<RefCell<Option<(CellAction, usize, usize)>>> =
        Rc::new(RefCell::new(None));

    // Pre-compute null status for context menu display.
    let is_null_fn = |r: usize, c: usize| -> bool {
        result
            .rows
            .get(r)
            .and_then(|row| row.get(c))
            .map(|v| v.is_null())
            .unwrap_or(false)
    };

    let available_height = ui.available_height() - 28.0;

    // Build columns: row-number + one per data column
    let mut builder = TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(ROW_NUM_WIDTH))
        .max_scroll_height(available_height)
        .sense(egui::Sense::click());

    for _ in 0..num_cols {
        builder = builder.column(
            Column::initial(160.0)
                .at_least(MIN_COL_WIDTH)
                .resizable(true)
                .clip(true),
        );
    }

    // Header
    let table = builder.header(ROW_HEIGHT, |mut header| {
        header.col(|ui| {
            ui.label(egui::RichText::new("#").strong().color(weak));
        });
        for col_meta in &result.columns {
            header.col(|ui| {
                ui.add(egui::Label::new(egui::RichText::new(&col_meta.name).strong()).truncate());
            });
        }
    });

    // Body — virtual scrolling via show_rows
    table.body(|body| {
        body.rows(ROW_HEIGHT, num_rows, |mut row| {
            let row_idx = row.index();
            let cached_row = match display_cache.get(row_idx) {
                Some(r) => r,
                None => return,
            };

            // Row number column — click to select entire row
            let is_deleted = pending.is_row_deleted(row_idx);
            let (_, row_num_resp) = row.col(|ui| {
                let is_row_selected = *selected_row == Some(row_idx);
                if is_deleted {
                    let rect = ui.max_rect();
                    ui.painter().rect_filled(rect, 0.0, delete_fill);
                } else if is_row_selected {
                    let rect = ui.max_rect();
                    ui.painter().rect_filled(rect, 0.0, selection_fill);
                }
                let row_num = cached_row.first().map(|s| s.as_str()).unwrap_or("");
                let text_color = if is_deleted { delete_text } else { weak };
                ui.label(egui::RichText::new(row_num).color(text_color));
            });
            if row_num_resp.clicked() {
                *selected_row = Some(row_idx);
                *selected_cell = None; // clear cell selection when row is selected
            }

            // Data columns
            for col_idx in 0..num_cols {
                let is_cell_edited = pending.is_cell_edited(row_idx, col_idx);
                let (_, response) = row.col(|ui| {
                    let is_cell_selected = *selected_cell == Some((row_idx, col_idx));
                    let is_row_selected = *selected_row == Some(row_idx);
                    if is_deleted {
                        let rect = ui.max_rect();
                        ui.painter().rect_filled(rect, 0.0, delete_fill);
                    } else if is_cell_edited {
                        let rect = ui.max_rect();
                        ui.painter().rect_filled(rect, 0.0, edit_fill);
                    } else if is_cell_selected || is_row_selected {
                        let rect = ui.max_rect();
                        ui.painter().rect_filled(rect, 0.0, selection_fill);
                    }

                    // Show edited value if pending, otherwise show original
                    let display_val =
                        if let Some(edited) = pending.get_edited_value(row_idx, col_idx) {
                            edited.new_value.display()
                        } else {
                            cached_row
                                .get(col_idx + 1)
                                .map(|s| s.as_str())
                                .unwrap_or("")
                                .to_string()
                        };

                    let text = if is_deleted {
                        egui::RichText::new(&display_val)
                            .strikethrough()
                            .color(delete_text)
                    } else {
                        egui::RichText::new(&display_val)
                    };
                    ui.add(egui::Label::new(text).selectable(false).truncate());
                });

                if response.clicked() {
                    *selected_cell = Some((row_idx, col_idx));
                    *selected_row = None; // clear row selection when cell is clicked
                }
                if response.double_clicked() {
                    output.double_clicked = Some((row_idx, col_idx));
                }
                // Right-click selects the cell and opens context menu.
                if response.secondary_clicked() {
                    *selected_cell = Some((row_idx, col_idx));
                }

                // Context menu — rendered per-cell, writes into shared Rc<RefCell>.
                let action_ref = Rc::clone(&pending_action);
                let is_null = is_null_fn(row_idx, col_idx);
                response.context_menu(|ui| {
                    ui.set_min_width(180.0);
                    render_cell_context_menu(ui, row_idx, col_idx, is_null, &action_ref);
                });
            }
        });
    });

    // Collect any pending action from the context menu.
    if let Some(act) = pending_action.borrow_mut().take() {
        output.action = Some(act);
    }

    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(format!(
            "{} rows  ·  {:.1} ms",
            num_rows,
            result.execution_time.as_secs_f64() * 1000.0
        ))
        .color(weak)
        .small(),
    );

    output
}
