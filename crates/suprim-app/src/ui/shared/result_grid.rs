/// Shared result-grid renderer — used by both SqlEditorTab and TableViewerTab.
/// Uses egui_extras::TableBuilder for responsive column widths and built-in virtual scrolling.
///
/// Context-menu rendering is in `result_grid_context_menu.rs`.
use eframe::egui;
use egui_extras::{Column, TableBuilder};
use std::cell::RefCell;
use std::rc::Rc;
use suprim_core::db::types::QueryResult;

use super::result_grid_context_menu::render_cell_context_menu;
use crate::ui::table_viewer_tab::column_filter::ColumnFilterState;
use crate::ui::table_viewer_tab::pending_changes::PendingChanges;
use crate::ui::table_viewer_tab::sort_state::SortState;

// ── Cell context-menu actions ─────────────────────────────────────────────────

/// Actions returned by the result grid when the user interacts via context menu.
#[derive(Debug, Clone)]
pub enum CellAction {
    Copy,
    CopyAsJson,
    CopyAsCsv,
    CopyAsSql,
    Paste,
    SetNull,
    SetEmpty,
    SetDefault,
    DuplicateRow,
    DeleteRow,
    EditValue,
}

/// Output of `render_result_grid`.
pub struct GridOutput {
    pub double_clicked: Option<(usize, usize)>,
    pub action: Option<(CellAction, usize, usize)>,
    pub sort_clicked: Option<(String, bool)>,
    pub filter_clicked: Option<(String, String, egui::Pos2)>,
}

/// Pre-compute display strings for all cells once (avoids per-frame allocations).
pub fn build_display_cache(result: &QueryResult) -> Vec<Vec<String>> {
    result
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut cached = Vec::with_capacity(row.len() + 1);
            cached.push(format!("{}", i + 1));
            for v in row {
                cached.push(v.display());
            }
            cached
        })
        .collect()
}

const MIN_COL_WIDTH: f32 = 80.0;
const ROW_NUM_WIDTH: f32 = 44.0;
const ROW_HEIGHT: f32 = 22.0;

/// Render the result grid.
///
/// - `sort_state: Some(...)` → column headers clickable for sort.
/// - `column_filters: Some(...)` → filter funnel icon on headers.
/// - Both `None` → plain read-only headers (SqlEditorTab).
pub fn render_result_grid(
    ui: &mut egui::Ui,
    result: &QueryResult,
    display_cache: &[Vec<String>],
    selected_cell: &mut Option<(usize, usize)>,
    selected_row: &mut Option<usize>,
    pending: &PendingChanges,
    sort_state: Option<&SortState>,
    column_filters: Option<&ColumnFilterState>,
) -> GridOutput {
    let mut output = GridOutput {
        double_clicked: None,
        action: None,
        sort_clicked: None,
        filter_clicked: None,
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

    // Cmd+C to copy selected cell
    if let Some((row, col)) = *selected_cell {
        let copy = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
        if copy {
            if let Some(db_val) = result.rows.get(row).and_then(|r| r.get(col)) {
                ui.ctx().copy_text(db_val.display());
            }
        }
    }

    let pending_action: super::result_grid_context_menu::CellActionRef =
        Rc::new(RefCell::new(None));
    let sort_click_cell: Rc<RefCell<Option<(String, bool)>>> = Rc::new(RefCell::new(None));
    let filter_click_cell: Rc<RefCell<Option<(String, String, egui::Pos2)>>> =
        Rc::new(RefCell::new(None));

    let is_null_fn = |r: usize, c: usize| -> bool {
        result
            .rows
            .get(r)
            .and_then(|row| row.get(c))
            .map(|v| v.is_null())
            .unwrap_or(false)
    };

    let available_height = ui.available_height() - 28.0;
    let interactive = sort_state.is_some() || column_filters.is_some();

    // Horizontal scroll for wide tables.
    egui::ScrollArea::horizontal()
        .id_salt("result_grid_hscroll")
        .show(ui, |ui| {
            let mut builder = TableBuilder::new(ui)
                .striped(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(Column::exact(ROW_NUM_WIDTH))
                .max_scroll_height(available_height);

            for _ in 0..num_cols {
                builder = builder.column(
                    Column::initial(160.0)
                        .at_least(MIN_COL_WIDTH)
                        .resizable(true)
                        .clip(true),
                );
            }

            let table = builder.header(ROW_HEIGHT, |mut header| {
                // Row number column
                header.col(|ui| {
                    ui.label(egui::RichText::new("#").strong().color(weak));
                });

                for (_ci, col_meta) in result.columns.iter().enumerate() {
                    header.col(|ui| {
                        if !interactive {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&col_meta.name).strong(),
                                )
                                .truncate(),
                            );
                            return;
                        }

                        // Interactive header — layout: [name] [sort arrow] ... [filter icon]
                        let cell_rect = ui.max_rect();
                        ui.horizontal(|ui| {
                            // Column name
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&col_meta.name).strong(),
                                )
                                .truncate(),
                            );

                            // Sort arrow
                            if let Some(ss) = sort_state {
                                if let Some(dir) = ss.direction(&col_meta.name) {
                                    let arrow = match dir {
                                        crate::ui::table_viewer_tab::sort_state::SortDirection::Asc => {
                                            egui_phosphor::regular::CARET_UP
                                        }
                                        crate::ui::table_viewer_tab::sort_state::SortDirection::Desc => {
                                            egui_phosphor::regular::CARET_DOWN
                                        }
                                    };
                                    ui.label(
                                        egui::RichText::new(arrow)
                                            .size(12.0)
                                            .color(selection_fill),
                                    );
                                    if ss.columns.len() > 1 {
                                        if let Some(pri) = ss.priority(&col_meta.name) {
                                            ui.label(
                                                egui::RichText::new(format!("{pri}"))
                                                    .small()
                                                    .weak(),
                                            );
                                        }
                                    }
                                }
                            }

                            // Filter icon — right-aligned, painted directly (no widget = no debug flash)
                            if column_filters.is_some() {
                                let has_active = column_filters
                                    .map(|cf| cf.has_filter(&col_meta.name))
                                    .unwrap_or(false);
                                let color = if has_active {
                                    selection_fill
                                } else {
                                    weak.gamma_multiply(0.4)
                                };
                                // Paint funnel icon at right edge of cell_rect
                                let icon_x = cell_rect.right() - 16.0;
                                let icon_center =
                                    egui::pos2(icon_x, cell_rect.center().y);
                                let galley = ui.painter().layout_no_wrap(
                                    egui_phosphor::regular::FUNNEL_SIMPLE.to_string(),
                                    egui::FontId::proportional(12.0),
                                    color,
                                );
                                let text_pos =
                                    icon_center - galley.size() / 2.0;
                                ui.painter().galley(text_pos, galley, color);
                            }
                        });

                        // Detect click on header cell via pointer (no extra Sense widget)
                        let hovered = ui.rect_contains_pointer(cell_rect);
                        if hovered {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        let clicked = hovered
                            && ui.input(|i| i.pointer.any_pressed());
                        if clicked {
                            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                                let filter_zone = cell_rect.right() - ROW_HEIGHT;
                                if column_filters.is_some() && pos.x >= filter_zone {
                                    let anchor = egui::pos2(
                                        cell_rect.left(),
                                        cell_rect.bottom() + 2.0,
                                    );
                                    *filter_click_cell.borrow_mut() = Some((
                                        col_meta.name.clone(),
                                        col_meta.db_type.clone(),
                                        anchor,
                                    ));
                                } else if sort_state.is_some() {
                                    let is_multi = ui.input(|i| i.modifiers.command);
                                    *sort_click_cell.borrow_mut() =
                                        Some((col_meta.name.clone(), is_multi));
                                }
                            }
                        }
                    });
                }
            });

            // Body
            table.body(|body| {
                body.rows(ROW_HEIGHT, num_rows, |mut row| {
                    let row_idx = row.index();
                    let cached_row = match display_cache.get(row_idx) {
                        Some(r) => r,
                        None => return,
                    };

                    let is_deleted = pending.is_row_deleted(row_idx);

                    // Row number column
                    row.col(|ui| {
                        let cell_rect = ui.max_rect();
                        let is_row_selected = *selected_row == Some(row_idx);
                        if is_deleted {
                            ui.painter()
                                .rect_filled(cell_rect, 0.0, delete_fill);
                        } else if is_row_selected {
                            ui.painter()
                                .rect_filled(cell_rect, 0.0, selection_fill);
                        }
                        let num = cached_row.first().map(|s| s.as_str()).unwrap_or("");
                        let color = if is_deleted { delete_text } else { weak };
                        ui.label(egui::RichText::new(num).color(color));
                        let resp = ui.interact(
                            cell_rect,
                            egui::Id::new(("row_num", row_idx)),
                            egui::Sense::click(),
                        );
                        if resp.clicked() {
                            *selected_row = Some(row_idx);
                            *selected_cell = None;
                        }
                    });

                    // Data columns
                    for col_idx in 0..num_cols {
                        let is_edited = pending.is_cell_edited(row_idx, col_idx);
                        row.col(|ui| {
                            let cell_rect = ui.max_rect();
                            let is_cell_sel = *selected_cell == Some((row_idx, col_idx));
                            let is_row_sel = *selected_row == Some(row_idx);
                            if is_deleted {
                                ui.painter()
                                    .rect_filled(cell_rect, 0.0, delete_fill);
                            } else if is_edited {
                                ui.painter()
                                    .rect_filled(cell_rect, 0.0, edit_fill);
                            } else if is_cell_sel || is_row_sel {
                                ui.painter()
                                    .rect_filled(cell_rect, 0.0, selection_fill);
                            }

                            let val = if let Some(ed) =
                                pending.get_edited_value(row_idx, col_idx)
                            {
                                ed.new_value.display()
                            } else {
                                cached_row
                                    .get(col_idx + 1)
                                    .map(|s| s.as_str())
                                    .unwrap_or("")
                                    .to_string()
                            };

                            let text = if is_deleted {
                                egui::RichText::new(&val)
                                    .strikethrough()
                                    .color(delete_text)
                            } else {
                                egui::RichText::new(&val)
                            };
                            ui.add(egui::Label::new(text).selectable(false).truncate());

                            // Click detection inside cell — single source, no overlap
                            let resp = ui.interact(
                                cell_rect,
                                egui::Id::new(("cell", row_idx, col_idx)),
                                egui::Sense::click(),
                            );
                            if resp.clicked() {
                                *selected_cell = Some((row_idx, col_idx));
                                *selected_row = None;
                            }
                            if resp.double_clicked() {
                                output.double_clicked = Some((row_idx, col_idx));
                            }
                            if resp.secondary_clicked() {
                                *selected_cell = Some((row_idx, col_idx));
                            }
                            let action_ref = Rc::clone(&pending_action);
                            let is_null = is_null_fn(row_idx, col_idx);
                            resp.context_menu(|ui| {
                                ui.set_min_width(180.0);
                                render_cell_context_menu(
                                    ui, row_idx, col_idx, is_null, &action_ref,
                                );
                            });
                        });
                    }
                });
            });
        }); // end ScrollArea

    // Collect deferred actions
    if let Some(act) = pending_action.borrow_mut().take() {
        output.action = Some(act);
    }
    if let Some(click) = sort_click_cell.borrow_mut().take() {
        output.sort_clicked = Some(click);
    }
    if let Some(click) = filter_click_cell.borrow_mut().take() {
        output.filter_clicked = Some(click);
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
