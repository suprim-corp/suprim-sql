/// Shared result-grid renderer — used by both SqlEditorTab and TableViewerTab.
/// Uses egui_extras::TableBuilder for responsive column widths and built-in virtual scrolling.
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use suprim_sql::db::types::QueryResult;

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

/// Returns `Some((row, col))` when a cell is double-clicked (for opening an editor).
pub fn render_result_grid(
    ui: &mut egui::Ui,
    result: &QueryResult,
    display_cache: &[Vec<String>],
    selected_cell: &mut Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    let mut double_clicked_cell: Option<(usize, usize)> = None;
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
                    double_clicked_cell = Some((row_idx, col_idx));
                }
            }
        });
    });

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

    double_clicked_cell
}
