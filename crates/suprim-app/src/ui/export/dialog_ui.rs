//! Options panel (right pane) + tables tree container (left pane) rendering.

use eframe::egui;

use super::csv_plugin;
use super::json_plugin;
use super::tree_widgets::render_database_node;
use super::types::{ExportFormatId, ExportMode};
use super::ExportDialog;

impl ExportDialog {
    pub(super) fn render_tables_tree(&mut self, ui: &mut egui::Ui, max_h: f32) {
        let ExportMode::Tables { items, .. } = &mut self.mode else {
            return;
        };

        egui::ScrollArea::vertical()
            .id_salt("export_tree")
            .max_height(max_h)
            .auto_shrink(false)
            .show(ui, |ui| {
                for db in items.iter_mut() {
                    render_database_node(ui, db);
                }
            });
    }

    pub(super) fn render_options_panel(&mut self, ui: &mut egui::Ui) {
        // Format segmented picker
        ui.horizontal(|ui| {
            for fmt in ExportFormatId::all() {
                let selected = self.format == *fmt;
                let button =
                    egui::Button::new(egui::RichText::new(fmt.label()).color(if selected {
                        egui::Color32::WHITE
                    } else {
                        ui.visuals().text_color()
                    }))
                    .fill(if selected {
                        egui::Color32::from_rgb(59, 130, 246)
                    } else {
                        egui::Color32::TRANSPARENT
                    });
                if ui
                    .add(button)
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
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

        // Format-specific options
        match self.format {
            ExportFormatId::Csv => csv_plugin::render_options_ui(ui, &mut self.csv_opts),
            ExportFormatId::Json => json_plugin::render_options_ui(ui, &mut self.json_opts),
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
                egui::RichText::new(format!(".{}", self.format.extension()))
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
