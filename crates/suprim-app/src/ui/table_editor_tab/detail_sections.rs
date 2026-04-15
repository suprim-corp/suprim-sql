/// Read-only sections for indexes and foreign keys display.
use eframe::egui;
use suprim_core::db::types::{ForeignKeyNode, IndexNode};

pub fn render_indexes_section(indexes: &[IndexNode], ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new(format!(
            "{} Indexes ({})",
            egui_phosphor::regular::MAGNIFYING_GLASS,
            indexes.len()
        ))
        .strong()
        .size(14.0),
    );

    if indexes.is_empty() {
        ui.label("No indexes");
        return;
    }

    egui::Grid::new("table_editor_indexes")
        .num_columns(3)
        .striped(true)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.strong("Name");
            ui.strong("Unique");
            ui.strong("Columns");
            ui.end_row();

            for idx in indexes {
                ui.label(&idx.name);
                ui.label(if idx.is_unique { "YES" } else { "NO" });
                ui.label(idx.columns.join(", "));
                ui.end_row();
            }
        });
}

pub fn render_foreign_keys_section(foreign_keys: &[ForeignKeyNode], ui: &mut egui::Ui) {
    ui.label(
        egui::RichText::new(format!(
            "{} Foreign Keys ({})",
            egui_phosphor::regular::LINK,
            foreign_keys.len()
        ))
        .strong()
        .size(14.0),
    );

    if foreign_keys.is_empty() {
        ui.label("No foreign keys");
        return;
    }

    egui::Grid::new("table_editor_fks")
        .num_columns(3)
        .striped(true)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.strong("Name");
            ui.strong("Columns");
            ui.strong("References");
            ui.end_row();

            for fk in foreign_keys {
                ui.label(&fk.name);
                ui.label(fk.columns.join(", "));
                ui.label(format!("{} ({})", fk.ref_table, fk.ref_columns.join(", ")));
                ui.end_row();
            }
        });
}
