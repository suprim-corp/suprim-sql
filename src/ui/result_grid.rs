/// Shared result-grid renderer — used by both SqlEditorTab and TableViewerTab.
use eframe::egui;
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

/// Render a fixed-width, left-aligned, clipped cell inside a `horizontal` row.
/// Returns the allocated `Rect` so callers can detect clicks on it.
pub fn fixed_cell(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    add_content: impl FnOnce(&mut egui::Ui),
) -> egui::Rect {
    let parent_clip = ui.clip_rect();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let clip = rect.intersect(parent_clip);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.set_clip_rect(clip);
    child.with_layout(
        egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(false),
        |ui| {
            add_content(ui);
        },
    );
    rect
}

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
    let col_width: f32 = 160.0;
    let row_num_width: f32 = 40.0;
    let row_height: f32 = 22.0;

    let available = ui.available_height() - 28.0;
    let weak = ui.visuals().weak_text_color();
    let border = ui.visuals().widgets.noninteractive.bg_stroke.color;
    let selection_fill = ui.visuals().selection.bg_fill;

    let total_width = row_num_width + (num_cols as f32 * col_width);

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

    egui::ScrollArea::horizontal()
        .id_salt("result_hscroll")
        .show(ui, |ui| {
            ui.set_min_width(total_width);

            // ── Header row ──
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                fixed_cell(ui, row_num_width, row_height, |ui| {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("#").strong().color(weak));
                });
                for col in &result.columns {
                    fixed_cell(ui, col_width, row_height, |ui| {
                        let rect = ui.max_rect();
                        ui.painter().vline(
                            rect.left(),
                            rect.y_range(),
                            egui::Stroke::new(1.0, border),
                        );
                        ui.add_space(6.0);
                        ui.add(
                            egui::Label::new(egui::RichText::new(&col.name).strong()).truncate(),
                        );
                    });
                }
            });

            // ── Separator ──
            let sep_rect = ui.available_rect_before_wrap();
            ui.painter().hline(
                sep_rect.left()..=sep_rect.left() + total_width,
                sep_rect.top(),
                egui::Stroke::new(1.0, border),
            );
            ui.add_space(1.0);

            // ── Data rows (virtual) ──
            egui::ScrollArea::vertical()
                .id_salt("result_vscroll")
                .max_height(available)
                .show_rows(ui, row_height, num_rows, |ui, row_range| {
                    for row_idx in row_range {
                        if let Some(cached_row) = display_cache.get(row_idx) {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                let row_num = cached_row.first().map(|s| s.as_str()).unwrap_or("");
                                fixed_cell(ui, row_num_width, row_height, |ui| {
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new(row_num).color(weak));
                                });
                                for col_idx in 0..num_cols {
                                    let val = cached_row
                                        .get(col_idx + 1)
                                        .map(|s| s.as_str())
                                        .unwrap_or("");
                                    let is_selected = *selected_cell == Some((row_idx, col_idx));
                                    let cell_rect = fixed_cell(ui, col_width, row_height, |ui| {
                                        let rect = ui.max_rect();
                                        // Draw selection bg BEFORE text
                                        if is_selected {
                                            ui.painter().rect_filled(rect, 0.0, selection_fill);
                                        }
                                        ui.painter().vline(
                                            rect.left(),
                                            rect.y_range(),
                                            egui::Stroke::new(1.0, border),
                                        );
                                        ui.add_space(6.0);
                                        ui.add(egui::Label::new(val).selectable(false).truncate());
                                    });
                                    // Click detection + pointer cursor
                                    let pointer_pos = ui.input(|i| i.pointer.interact_pos());
                                    if let Some(pos) = pointer_pos {
                                        if cell_rect.contains(pos) {
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                                            if ui.input(|i| i.pointer.any_click()) {
                                                *selected_cell = Some((row_idx, col_idx));
                                            }
                                            if ui.input(|i| {
                                                i.pointer.button_double_clicked(
                                                    egui::PointerButton::Primary,
                                                )
                                            }) {
                                                double_clicked_cell = Some((row_idx, col_idx));
                                            }
                                        }
                                    }
                                }
                            });
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
