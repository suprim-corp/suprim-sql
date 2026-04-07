/// Shared result-grid renderer — used by both SqlEditorTab and TableViewerTab.
/// Uses egui_extras::TableBuilder for responsive column widths and built-in virtual scrolling.
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::cell::RefCell;
use std::rc::Rc;
use suprim_sql::db::types::QueryResult;

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
) -> GridOutput {
    let mut output = GridOutput {
        double_clicked: None,
        action: None,
    };
    let num_rows = result.rows.len();
    let num_cols = result.columns.len();
    let weak = ui.visuals().weak_text_color();
    let selection_fill = ui.visuals().selection.bg_fill;

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

            // Row number column
            row.col(|ui| {
                let row_num = cached_row.first().map(|s| s.as_str()).unwrap_or("");
                ui.label(egui::RichText::new(row_num).color(weak));
            });

            // Data columns
            for col_idx in 0..num_cols {
                let (_, response) = row.col(|ui| {
                    let is_selected = *selected_cell == Some((row_idx, col_idx));
                    if is_selected {
                        let rect = ui.max_rect();
                        ui.painter().rect_filled(rect, 0.0, selection_fill);
                    }
                    let val = cached_row
                        .get(col_idx + 1)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    ui.add(egui::Label::new(val).selectable(false).truncate());
                });

                if response.clicked() {
                    *selected_cell = Some((row_idx, col_idx));
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

/// Render the context-menu items for a cell.
fn render_cell_context_menu(
    ui: &mut egui::Ui,
    row: usize,
    col: usize,
    is_null: bool,
    action_ref: &Rc<RefCell<Option<(CellAction, usize, usize)>>>,
) {
    // ── Copy ──
    if ui
        .add(egui::Button::new("Copy").shortcut_text("⌘C"))
        .clicked()
    {
        *action_ref.borrow_mut() = Some((CellAction::Copy, row, col));
        ui.close();
    }

    ui.menu_button("Copy as", |ui| {
        if ui.button("JSON").clicked() {
            *action_ref.borrow_mut() = Some((CellAction::CopyAsJson, row, col));
            ui.close();
        }
        if ui.button("CSV").clicked() {
            *action_ref.borrow_mut() = Some((CellAction::CopyAsCsv, row, col));
            ui.close();
        }
        if ui.button("SQL").clicked() {
            *action_ref.borrow_mut() = Some((CellAction::CopyAsSql, row, col));
            ui.close();
        }
    });

    // ── Paste ──
    if ui
        .add(egui::Button::new("Paste").shortcut_text("⌘V"))
        .clicked()
    {
        *action_ref.borrow_mut() = Some((CellAction::Paste, row, col));
        ui.close();
    }

    ui.separator();

    // ── Set Value ──
    ui.menu_button("Set Value", |ui| {
        let null_label = if is_null {
            egui::RichText::new("NULL  ✓")
        } else {
            egui::RichText::new("NULL")
        };
        if ui.button(null_label).clicked() {
            *action_ref.borrow_mut() = Some((CellAction::SetNull, row, col));
            ui.close();
        }
        if ui.button("Empty String").clicked() {
            *action_ref.borrow_mut() = Some((CellAction::SetEmpty, row, col));
            ui.close();
        }
        if ui.button("Default").clicked() {
            *action_ref.borrow_mut() = Some((CellAction::SetDefault, row, col));
            ui.close();
        }
    });

    // ── Edit Value ──
    if ui.button("Edit Value...").clicked() {
        *action_ref.borrow_mut() = Some((CellAction::EditValue, row, col));
        ui.close();
    }

    ui.separator();

    // ── Export Results ──
    if ui.button("Export Results...").clicked() {
        *action_ref.borrow_mut() = Some((CellAction::ExportResults, row, col));
        ui.close();
    }

    // ── Duplicate Row ──
    if ui
        .add(egui::Button::new("Duplicate").shortcut_text("⌘D"))
        .clicked()
    {
        *action_ref.borrow_mut() = Some((CellAction::DuplicateRow, row, col));
        ui.close();
    }

    // ── Delete Row ──
    let delete_label = egui::RichText::new("Delete").color(egui::Color32::from_rgb(220, 60, 60));
    if ui
        .add(egui::Button::new(delete_label).shortcut_text("⌫"))
        .clicked()
    {
        *action_ref.borrow_mut() = Some((CellAction::DeleteRow, row, col));
        ui.close();
    }
}
