//! Options panel (right pane) + tables tree container (left pane) rendering.

use eframe::egui;

use super::csv_plugin;
use super::json_plugin;
use super::sql_plugin;
use super::tree_widgets::render_database_node;
use super::types::{ExportFormatId, ExportMode};
use super::ExportDialog;

/// Fixed width for each SQL toggle column (Structure / Drop / Data).
pub(super) const SQL_COL_W: f32 = 48.0;

/// Render the 3 SQL column headers ("Structure", "Drop", "Data") right-aligned.
/// Call this inside a horizontal layout — it consumes remaining width.
pub(super) fn render_sql_column_headers(ui: &mut egui::Ui) {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        // Right-to-left: Data, Drop, Structure (reversed order)
        ui.allocate_ui_with_layout(
            egui::vec2(SQL_COL_W, ui.available_height()),
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                ui.label(egui::RichText::new("Data").weak().size(11.0));
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(SQL_COL_W, ui.available_height()),
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                ui.label(egui::RichText::new("Drop").weak().size(11.0));
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(SQL_COL_W, ui.available_height()),
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| {
                ui.label(egui::RichText::new("Structure").weak().size(11.0));
            },
        );
    });
}

impl ExportDialog {
    pub(super) fn render_tables_tree(&mut self, ui: &mut egui::Ui, max_h: f32) {
        let format = self.format;
        let ExportMode::Tables { items, .. } = &mut self.mode else {
            return;
        };

        egui::ScrollArea::vertical()
            .id_salt("export_tree")
            .max_height(max_h)
            .auto_shrink(false)
            .show(ui, |ui| {
                for db in items.iter_mut() {
                    render_database_node(ui, db, format);
                }
            });
    }

    pub(super) fn render_options_panel(&mut self, ui: &mut egui::Ui) {
        // Format segmented picker
        ui.horizontal(|ui| {
            for fmt in ExportFormatId::all() {
                let selected = self.format == *fmt;
                let available = fmt.is_available();

                let text = if available {
                    egui::RichText::new(fmt.label())
                } else {
                    egui::RichText::new(format!("{}", fmt.label())).weak()
                };
                let text = text.color(if selected {
                    egui::Color32::WHITE
                } else if available {
                    ui.visuals().text_color()
                } else {
                    ui.visuals().weak_text_color()
                });

                let button = egui::Button::new(text).fill(if selected {
                    egui::Color32::from_rgb(59, 130, 246)
                } else {
                    egui::Color32::TRANSPARENT
                });

                let resp = ui.add_enabled(available, button);
                let resp = if available {
                    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
                } else {
                    resp.on_hover_text("Coming soon")
                };
                if resp.clicked() && available {
                    self.format = *fmt;
                }
            }
        });

        ui.add_space(8.0);

        // Format description
        ui.label(
            egui::RichText::new(self.format.description())
                .weak()
                .size(12.0),
        );

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        // Format-specific options (only rendered for available formats)
        match self.format {
            ExportFormatId::Csv => csv_plugin::render_options_ui(ui, &mut self.csv_opts),
            ExportFormatId::Json => json_plugin::render_options_ui(ui, &mut self.json_opts),
            ExportFormatId::Sql => sql_plugin::render_options_ui(ui, &mut self.sql_opts),
            ExportFormatId::Xlsx => {
                ui.vertical_centered(|ui| {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new(format!(
                            "{}  Coming soon",
                            egui_phosphor::regular::HOURGLASS
                        ))
                        .weak()
                        .size(14.0),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("XLSX export is not yet available.")
                            .weak()
                            .size(12.0),
                    );
                });
            }
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        // File name field
        ui.label(egui::RichText::new("File name").strong());
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.file_name)
                    .desired_width(220.0)
                    .hint_text("export"),
            );
            ui.label(
                egui::RichText::new(format!(".{}", self.full_extension()))
                    .weak()
                    .monospace(),
            );
        });

        // Inline error below filename
        if let Some(err) = &self.error {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(err)
                    .color(egui::Color32::from_rgb(220, 60, 60))
                    .size(11.0),
            );
        }
    }
}
