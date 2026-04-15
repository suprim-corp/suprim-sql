/// New-row editor — popup dialog for inserting a blank row with column inputs.
use eframe::egui;
use suprim_core::db::types::{ColumnMeta, DbValue};
use uuid::Uuid;

use super::sql_preview;
use super::TableViewerTab;

/// State for the new-row editor popup.
pub(super) struct NewRowEditor {
    /// Column definitions from the current query result.
    pub columns: Vec<ColumnMeta>,
    /// User-entered values — one per column. Empty string = use DEFAULT.
    pub values: Vec<String>,
    /// Which columns are explicitly set to NULL (overrides empty = DEFAULT).
    pub is_null: Vec<bool>,
    /// Cached SQL preview string.
    pub sql_preview: String,
    /// Whether to show the SQL preview panel.
    pub show_preview: bool,
}

impl NewRowEditor {
    pub fn new(columns: Vec<ColumnMeta>) -> Self {
        let count = columns.len();
        Self {
            columns,
            values: vec![String::new(); count],
            is_null: vec![false; count],
            sql_preview: String::new(),
            show_preview: false,
        }
    }

    /// Build the (column_name, DbValue) pairs for non-empty fields.
    fn build_values(&self) -> Vec<(String, DbValue)> {
        let mut result = Vec::new();
        for (i, col) in self.columns.iter().enumerate() {
            if self.is_null[i] {
                result.push((col.name.clone(), DbValue::Null));
            } else if !self.values[i].is_empty() {
                result.push((col.name.clone(), DbValue::Text(self.values[i].clone())));
            }
            // Empty + not null = skip (use DEFAULT)
        }
        result
    }

    /// Regenerate the SQL preview from current values.
    fn regenerate_preview(&mut self, schema: &str, table: &str) {
        let vals = self.build_values();
        self.sql_preview = sql_preview::preview_insert_sql(schema, table, &vals);
    }
}

impl TableViewerTab {
    /// Render the new-row editor popup window.
    pub(super) fn render_new_row_editor(&mut self, ctx: &egui::Context, _tab_id: Uuid) {
        let editor = match &mut self.new_row_editor {
            Some(e) => e,
            None => return,
        };

        let mut open = true;
        let mut do_insert = false;
        let mut do_cancel = false;

        egui::Window::new("Insert New Row")
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(500.0)
            .default_height(400.0)
            .open(&mut open)
            .show(ctx, |ui| {
                // Column input grid
                let grid_height = if editor.show_preview {
                    ui.available_height() - 180.0
                } else {
                    ui.available_height() - 36.0
                };

                egui::ScrollArea::vertical()
                    .id_salt("new_row_scroll")
                    .max_height(grid_height.max(100.0))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("new_row_grid")
                            .num_columns(4)
                            .spacing([8.0, 4.0])
                            .striped(true)
                            .min_col_width(0.0)
                            .show(ui, |ui| {
                                // Header
                                ui.label(egui::RichText::new("Column").strong());
                                ui.label(egui::RichText::new("Type").strong().weak());
                                ui.label(egui::RichText::new("NULL").strong());
                                ui.label(egui::RichText::new("Value").strong());
                                ui.end_row();

                                for i in 0..editor.columns.len() {
                                    let col_name = editor.columns[i].name.clone();
                                    let col_type = editor.columns[i].db_type.clone();

                                    ui.label(&col_name);
                                    ui.label(egui::RichText::new(&col_type).weak().small());

                                    // NULL checkbox
                                    if ui.checkbox(&mut editor.is_null[i], "").changed()
                                        && editor.is_null[i]
                                    {
                                        editor.values[i].clear();
                                    }

                                    // Value input (disabled if NULL) — fill remaining width
                                    let value_width = ui.available_width();
                                    ui.add_enabled(
                                        !editor.is_null[i],
                                        egui::TextEdit::singleline(&mut editor.values[i])
                                            .desired_width(value_width)
                                            .hint_text("DEFAULT"),
                                    );
                                    ui.end_row();
                                }
                            });
                    });

                // SQL Preview toggle + panel
                ui.separator();
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(editor.show_preview, "SQL Preview")
                        .clicked()
                    {
                        editor.show_preview = !editor.show_preview;
                        if editor.show_preview {
                            editor.regenerate_preview(
                                &self.schema_name.clone(),
                                &self.table_name.clone(),
                            );
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(egui::RichText::new("Insert").strong())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            do_insert = true;
                        }
                        if ui
                            .button("Cancel")
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            do_cancel = true;
                        }
                    });
                });

                if editor.show_preview {
                    // Regenerate each frame when preview is visible
                    editor.regenerate_preview(&self.schema_name.clone(), &self.table_name.clone());

                    egui::ScrollArea::vertical()
                        .id_salt("new_row_sql_preview")
                        .max_height(120.0)
                        .show(ui, |ui| {
                            let preview = editor.sql_preview.clone();
                            ui.add(
                                egui::TextEdit::multiline(&mut preview.as_str())
                                    .code_editor()
                                    .desired_width(f32::INFINITY),
                            );
                        });
                }
            });

        if do_insert {
            self.buffer_insert_new_row();
            self.new_row_editor = None;
        } else if !open || do_cancel {
            self.new_row_editor = None;
        }
    }

    /// Buffer the new row into pending changes (does NOT send to DB).
    fn buffer_insert_new_row(&mut self) {
        let editor = match &self.new_row_editor {
            Some(e) => e,
            None => return,
        };

        let pairs = editor.build_values();
        let mut values = std::collections::HashMap::new();
        for (col_name, val) in pairs {
            values.insert(col_name, val);
        }

        self.pending.add_row(values);
    }
}
