//! Step 4: Preview — read-only DDL script viewer.
//!
//! Displays the generated DDL in a scrollable, read-only text area
//! with line numbers and monospace font.

use eframe::egui;

/// Render the DDL preview panel.
///
/// Shows a read-only, scrollable text area with the full DDL script.
/// When the script is empty, displays a "no changes" message.
pub(crate) fn render_ddl_preview(ui: &mut egui::Ui, ddl_script: &str, max_height: f32) {
    if ddl_script.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(max_height / 3.0);
            ui.label(
                egui::RichText::new("No DDL statements to preview.")
                    .weak()
                    .size(13.0),
            );
            ui.label(
                egui::RichText::new("All entries may be unchecked, or schemas are identical.")
                    .weak()
                    .size(11.0),
            );
        });
        return;
    }

    let line_count = ddl_script.lines().count();
    let line_count_label = format!("{} line(s)", line_count);

    // Header with line count
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{}  DDL Script Preview",
                egui_phosphor::regular::FILE_SQL
            ))
            .strong()
            .size(12.0),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(&line_count_label).weak().size(11.0));
        });
    });

    ui.add_space(2.0);

    // Scrollable read-only text area with monospace font
    let available_h = (max_height - 22.0).max(40.0); // 22px for header + spacing
    egui::ScrollArea::both()
        .id_salt("ddl_preview_scroll")
        .max_height(available_h)
        .show(ui, |ui| {
            let mut script_text = ddl_script.to_owned();
            ui.add(
                egui::TextEdit::multiline(&mut script_text)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .interactive(false)
                    .code_editor(),
            );
        });
}
