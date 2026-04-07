/// Cell editor sub-widgets — JSON editor, plain text editor, header, buttons.
use eframe::egui;

use super::cell_editor::{CellEditor, CellEditorAction};
use super::TableViewerTab;
use crate::ui::editor_themes::adaptive_code_theme;

impl TableViewerTab {
    pub(super) fn render_editor_header(ui: &mut egui::Ui, col_name: &str, is_json: bool) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Column: {col_name}"))
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
            if is_json {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("JSON")
                            .small()
                            .color(egui::Color32::from_rgb(86, 156, 214)),
                    );
                });
            }
        });
    }

    pub(super) fn render_json_editor(ui: &mut egui::Ui, edit_value: &mut String, text_height: f32) {
        use egui_code_editor::{CodeEditor, Syntax};

        let theme = adaptive_code_theme(ui.visuals().dark_mode);
        let json_syntax = Syntax::new("json")
            .with_case_sensitive(true)
            .with_keywords(["true", "false", "null"])
            .with_quotes(['"']);

        egui::ScrollArea::vertical()
            .max_height(text_height)
            .auto_shrink(false)
            .show(ui, |ui| {
                CodeEditor::default()
                    .id_source("json_cell_editor")
                    .with_rows(12)
                    .with_fontsize(13.0)
                    .with_theme(theme)
                    .with_syntax(json_syntax)
                    .with_numlines(true)
                    .vscroll(false)
                    .show(ui, edit_value);
            });
    }

    pub(super) fn render_plain_editor(
        ui: &mut egui::Ui,
        edit_value: &mut String,
        text_height: f32,
    ) {
        egui::ScrollArea::vertical()
            .max_height(text_height)
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_sized(
                    [ui.available_width(), text_height],
                    egui::TextEdit::multiline(edit_value).font(egui::TextStyle::Monospace),
                );
            });
    }

    pub(super) fn render_editor_buttons(
        ui: &mut egui::Ui,
        editor: &mut CellEditor,
        is_json: bool,
        action: &mut CellEditorAction,
    ) {
        ui.horizontal(|ui| {
            let changed = editor.edit_value != editor.original_value;
            if is_json {
                if ui.button("Format").clicked() {
                    if let Ok(parsed) =
                        serde_json::from_str::<serde_json::Value>(&editor.edit_value)
                    {
                        editor.edit_value = serde_json::to_string_pretty(&parsed)
                            .unwrap_or(editor.edit_value.clone());
                        editor.json_error = None;
                    } else {
                        editor.json_error = Some("Invalid JSON — cannot format".into());
                    }
                }
            }
            if ui.add_enabled(changed, egui::Button::new("Save")).clicked() {
                if is_json {
                    match serde_json::from_str::<serde_json::Value>(&editor.edit_value) {
                        Ok(_) => {
                            editor.json_error = None;
                            *action = CellEditorAction::Save;
                        }
                        Err(e) => {
                            editor.json_error = Some(format!("Invalid JSON: {e}"));
                        }
                    }
                } else {
                    *action = CellEditorAction::Save;
                }
            }
            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                *action = CellEditorAction::Close;
            }
        });
    }
}
