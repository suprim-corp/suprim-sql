/// SQL editor autocomplete integration — handles auto-pair brackets and keyword popup.
/// Extracted from `sql_editor_tab.rs` to reduce file size.
use eframe::egui;

use super::sql_autocomplete;
use super::sql_highlighter;
use super::SqlEditorTab;

impl SqlEditorTab {
    /// Render the SQL text editor area with syntax highlighting + autocomplete.
    /// Returns the height consumed (for layout).
    pub(super) fn render_editor_area(&mut self, ui: &mut egui::Ui, editor_height: f32) {
        // Collect input events BEFORE rendering the editor (needed for auto-pair).
        let input_events: Vec<egui::Event> = ui.input(|i| i.events.clone());

        // Phase 0: Consume autocomplete navigation keys BEFORE TextEdit
        // so Enter/Tab/Arrow don't reach the editor.
        sql_autocomplete::consume_autocomplete_keys(ui, &mut self.autocomplete);

        let text_edit_id = egui::Id::new("sql_editor_textedit");
        let dark_mode = ui.visuals().dark_mode;
        let mono_font = egui::FontId::monospace(14.0);
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap_width: f32| {
            let job = sql_highlighter::sql_layout_job(text.as_str(), mono_font.clone(), dark_mode);
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let scroll_out = egui::ScrollArea::vertical()
            .id_salt("sql_editor_scroll")
            .max_height(editor_height)
            .show(ui, |ui| {
                egui::TextEdit::multiline(&mut self.sql_text)
                    .id(text_edit_id)
                    .font(egui::TextStyle::Monospace)
                    .desired_rows(10)
                    .desired_width(f32::INFINITY)
                    .hint_text("SELECT …")
                    .layouter(&mut layouter)
                    .show(ui)
            });

        let te_output = scroll_out.inner;

        // --- Auto-pair: insert matching close bracket ---
        let cursor_char_pos = te_output.cursor_range.map(|cr| cr.primary.index);

        if te_output.response.changed() {
            if let Some(pos) = cursor_char_pos {
                if sql_autocomplete::handle_auto_pair(&mut self.sql_text, Some(pos), &input_events)
                {
                    // Move cursor back between the pair.
                    if let Some(mut state) =
                        egui::TextEdit::load_state(ui.ctx(), te_output.response.id)
                    {
                        let cc = egui::text::CCursor::new(pos);
                        state
                            .cursor
                            .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                        state.store(ui.ctx(), te_output.response.id);
                    }
                }
            }
        }

        // --- Keyword autocomplete ---
        let has_focus = te_output.response.has_focus();
        if has_focus {
            if let Some(pos) = cursor_char_pos {
                sql_autocomplete::update_autocomplete(
                    &mut self.autocomplete,
                    &self.sql_text,
                    pos,
                    &[], // SQL editor: no extra column suggestions
                );
            }

            // Compute cursor screen position from galley for popup anchoring.
            let cursor_screen_pos = cursor_char_pos.map(|char_pos| {
                let ccursor = egui::text::CCursor::new(char_pos);
                let galley_offset = te_output.galley.pos_from_cursor(ccursor);
                let line_height = te_output
                    .galley
                    .rows
                    .first()
                    .map(|r| r.rect().height())
                    .unwrap_or(16.0);
                egui::pos2(
                    te_output.galley_pos.x + galley_offset.min.x,
                    te_output.galley_pos.y + galley_offset.min.y + line_height,
                )
            });

            if let Some(accepted) = sql_autocomplete::show_autocomplete_popup(
                ui,
                &mut self.autocomplete,
                text_edit_id,
                cursor_screen_pos,
            ) {
                let new_cursor = sql_autocomplete::apply_suggestion(
                    &mut self.sql_text,
                    accepted.prefix_char_start,
                    accepted.prefix_char_len,
                    &accepted.replacement,
                );
                // Update cursor position after replacement.
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), te_output.response.id)
                {
                    let cc = egui::text::CCursor::new(new_cursor);
                    state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                    state.store(ui.ctx(), te_output.response.id);
                }
                // Re-focus the editor.
                te_output.response.request_focus();
            }
        } else {
            self.autocomplete.close();
        }
    }
}
