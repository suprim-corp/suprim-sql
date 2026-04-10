/// Columns grid renderer using egui_extras::TableBuilder for full-width stretch.
use eframe::egui;
use egui_extras::{Column, TableBuilder};

use suprim_sql::db::drivers::postgres::{PG_COLUMN_TYPES, PG_TYPES_WITH_PARAMS};

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
    let col_length_w = 56.0_f32;
    let col_pk_w = 40.0_f32;
    let col_notnull_w = 70.0_f32;
    let col_trash_w = 30.0_f32;
    let fixed_total = col_num_w + col_length_w + col_pk_w + col_notnull_w + col_trash_w;

    // 8 columns = 7 gaps + scrollbar gutter (~16px)
    let spacing = ui.spacing().item_spacing.x;
    let overhead = fixed_total + spacing * 7.0 + 16.0;
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
        .column(Column::exact(col_length_w)) // Length
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
                ui.strong("Length");
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

                // #
                row.col(|ui| {
                    ui.label(format!("{}", idx + 1));
                });
                // Name
                row.col(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut col.name)
                            .desired_width(ui.available_width())
                            .hint_text("column_name"),
                    );
                });
                // Type (ComboBox)
                row.col(|ui| {
                    let combo_id = ui.make_persistent_id(format!("col_type_{idx}"));
                    egui::ComboBox::from_id_salt(combo_id)
                        .selected_text(&col.db_type)
                        .width(ui.available_width() - 8.0)
                        .show_ui(ui, |ui| {
                            for &t in PG_COLUMN_TYPES {
                                ui.selectable_value(&mut col.db_type, t.to_string(), t);
                            }
                        });
                });
                // Length / Precision
                row.col(|ui| {
                    let needs_param = PG_TYPES_WITH_PARAMS
                        .iter()
                        .any(|&t| t == col.db_type.as_str());
                    if needs_param {
                        ui.add(
                            egui::TextEdit::singleline(&mut col.type_param)
                                .desired_width(ui.available_width())
                                .hint_text("n"),
                        );
                    }
                });
                // PK
                row.col(|ui| {
                    ui.checkbox(&mut col.is_primary_key, "");
                });
                // NOT NULL
                row.col(|ui| {
                    let mut not_null = !col.nullable;
                    if ui.checkbox(&mut not_null, "").changed() {
                        col.nullable = !not_null;
                    }
                });
                // Default
                row.col(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut col.default_value)
                            .desired_width(ui.available_width())
                            .hint_text("default"),
                    );
                });
                // Trash
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
