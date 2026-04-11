//! Step 4: Preview — read-only DDL script viewer.
//!
//! Displays the generated DDL in a scrollable, read-only text area
//! with line numbers and monospace font.

use eframe::egui;

use crate::ui::sql_editor::sql_highlighter;

/// Render the DDL preview panel.
///
/// Shows a read-only, scrollable text area with the full DDL script
/// and SQL syntax highlighting (keywords, types, strings, comments, etc.).
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

    // Scrollable read-only text area with SQL syntax highlighting
    let available_h = (max_height - 22.0).max(40.0); // 22px for header + spacing
    let dark_mode = ui.visuals().dark_mode;
    let mono_font = egui::FontId::monospace(13.0);
    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap_width: f32| {
        let job = sql_highlighter::sql_layout_job(text.as_str(), mono_font.clone(), dark_mode);
        ui.fonts_mut(|f| f.layout_job(job))
    };

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
                    .code_editor()
                    .layouter(&mut layouter),
            );
        });
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    #[test]
    fn empty_script_shows_no_statements_message() {
        let harness = Harness::new_ui(|ui| {
            render_ddl_preview(ui, "", 300.0);
        });

        harness.get_by_label("No DDL statements to preview.");
    }

    #[test]
    fn non_empty_script_shows_line_count() {
        let script = "CREATE TABLE test (id INT);\nALTER TABLE test ADD col TEXT;";

        let harness = Harness::new_ui(|ui| {
            render_ddl_preview(ui, script, 300.0);
        });

        // Should show "2 line(s)" in the header
        harness.get_by_label("2 line(s)");
    }

    #[test]
    fn multiline_script_counts_lines_correctly() {
        let script =
            "-- Header\nCREATE TABLE a (id INT);\n\nCREATE TABLE b (id INT);\nDROP TABLE c;";

        let harness = Harness::new_ui(|ui| {
            render_ddl_preview(ui, script, 400.0);
        });

        harness.get_by_label("5 line(s)");
    }

    #[test]
    fn snapshot_ddl_preview_with_content() {
        let script = "-- Structure synchronization: public -> staging\n\
                       -- Generated at: 2026-04-11\n\n\
                       CREATE TABLE \"staging\".\"users\" (\n\
                       \t\"id\" bigint NOT NULL,\n\
                       \t\"name\" varchar(255),\n\
                       \tPRIMARY KEY (\"id\")\n\
                       );\n\n\
                       DROP TABLE IF EXISTS \"staging\".\"old_logs\" CASCADE;";

        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(600.0, 400.0))
            .build_ui(|ui| {
                render_ddl_preview(ui, script, 350.0);
            });

        harness.fit_contents();
        #[cfg(all(feature = "wgpu", feature = "snapshot"))]
        harness.snapshot("ddl_preview_with_content");
    }

    #[test]
    fn snapshot_ddl_preview_empty() {
        let mut harness = Harness::builder()
            .with_size(egui::Vec2::new(400.0, 200.0))
            .build_ui(|ui| {
                render_ddl_preview(ui, "", 150.0);
            });

        harness.fit_contents();
        #[cfg(all(feature = "wgpu", feature = "snapshot"))]
        harness.snapshot("ddl_preview_empty");
    }
}
