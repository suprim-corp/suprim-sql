/// Columns grid renderer using egui_extras::TableBuilder for full-width stretch.
use eframe::egui;
use egui_extras::{Column, TableBuilder};

use super::EditableColumn;

/// Renders the editable columns grid and returns the index of a column to remove (if any).
pub fn render_columns_grid(columns: &mut Vec<EditableColumn>, ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new(format!(
            "{} Columns ({})",
            egui_phosphor::regular::COLUMNS,
            columns.len()
        ))
        .strong()
        .size(14.0),
    );
    ui.add_space(4.0);

    let mut remove_idx: Option<usize> = None;

    let row_height = 24.0;
    let num_rows = columns.len();

    // Fixed column widths
    let col_num_w = 30.0_f32;
    let col_pk_w = 40.0_f32;
    let col_notnull_w = 70.0_f32;
    let col_trash_w = 30.0_f32;
    let fixed_total = col_num_w + col_pk_w + col_notnull_w + col_trash_w;

    // TableBuilder adds item_spacing.x between each column (7 cols = 6 gaps)
    // plus a scrollbar gutter (~16px). Account for all of it.
    let spacing = ui.spacing().item_spacing.x;
    let overhead = fixed_total + spacing * 6.0 + 16.0;
    // Distribute remaining width across Name(3), Type(2), Default(2) = 7 parts
    let available = (ui.available_width() - overhead).max(210.0);
    let name_w = (available * 3.0 / 7.0).max(80.0);
    let type_w = (available * 2.0 / 7.0).max(60.0);
    let default_w = (available * 2.0 / 7.0).max(60.0);

    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(col_num_w)) // #
        .column(Column::exact(name_w)) // Name
        .column(Column::exact(type_w)) // Type
        .column(Column::exact(col_pk_w)) // PK
        .column(Column::exact(col_notnull_w)) // NOT NULL
        .column(Column::exact(default_w)) // Default
        .column(Column::exact(col_trash_w)) // trash
        .header(row_height, |mut header| {
            header.col(|ui| {
                ui.strong("#");
            });
            header.col(|ui| {
                ui.strong("Name");
            });
            header.col(|ui| {
                ui.strong("Type");
            });
            header.col(|ui| {
                ui.strong("PK");
            });
            header.col(|ui| {
                ui.strong("NOT NULL");
            });
            header.col(|ui| {
                ui.strong("Default");
            });
            header.col(|_ui| {});
        })
        .body(|body| {
            body.rows(row_height, num_rows, |mut row| {
                let idx = row.index();
                let col = &mut columns[idx];

                row.col(|ui| {
                    ui.label(format!("{}", idx + 1));
                });
                row.col(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut col.name)
                            .desired_width(ui.available_width())
                            .hint_text("column_name"),
                    );
                });
                row.col(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut col.db_type)
                            .desired_width(ui.available_width())
                            .hint_text("type"),
                    );
                });
                row.col(|ui| {
                    ui.checkbox(&mut col.is_primary_key, "");
                });
                row.col(|ui| {
                    let mut not_null = !col.nullable;
                    if ui.checkbox(&mut not_null, "").changed() {
                        col.nullable = !not_null;
                    }
                });
                row.col(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut col.default_value)
                            .desired_width(ui.available_width())
                            .hint_text("default"),
                    );
                });
                row.col(|ui| {
                    let delete_label = egui::RichText::new(egui_phosphor::regular::TRASH)
                        .color(egui::Color32::from_rgb(220, 60, 60));
                    if ui
                        .button(delete_label)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        remove_idx = Some(idx);
                    }
                });
            });
        });

    if let Some(idx) = remove_idx {
        columns.remove(idx);
    }
}
