/// Filter bar — WHERE / ORDER BY input fields with reload button and autocomplete.
use eframe::egui;
use egui::text_edit::TextEditOutput;
use suprim_core::db::commands::DbCommand;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::sort_state::SortState;
use super::TableViewerTab;
use crate::ui::sql_editor::sql_autocomplete;
use crate::ui::sql_editor::sql_highlighter;

impl TableViewerTab {
    pub(super) fn render_filter_bar(
        &mut self,
        ui: &mut egui::Ui,
        tab_id: Uuid,
        cmd_tx: &mpsc::Sender<DbCommand>,
        bar_bg: egui::Color32,
        bar_stroke_color: egui::Color32,
        hint_color: egui::Color32,
    ) {
        egui::Frame::NONE
            .fill(bar_bg)
            .stroke(egui::Stroke::new(1.0, bar_stroke_color))
            .inner_margin(egui::Margin::symmetric(4, 3))
            .show(ui, |ui| {
                let _total_w = ui.available_width();
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;

                    // ── Toolbar icon group ──
                    let icon_color = hint_color;
                    let icon_size = 16.0;
                    self.render_toolbar_icons(ui, tab_id, cmd_tx, icon_color, icon_size);

                    ui.separator();

                    // WHERE section
                    let remaining = ui.available_width();
                    let where_w = (remaining * 0.55 - 50.0).max(80.0);
                    ui.label(egui::RichText::new("WHERE").color(hint_color).small());
                    // Active filter count badge
                    let fc = self.column_filters.active_count();
                    if fc > 0 {
                        ui.label(
                            egui::RichText::new(format!("({fc})"))
                                .color(egui::Color32::from_rgb(59, 130, 246))
                                .small()
                                .strong(),
                        );
                    }

                    let where_id = egui::Id::new("filter_where_text");
                    sql_autocomplete::consume_autocomplete_keys(ui, &mut self.where_autocomplete);
                    let dark_mode = ui.visuals().dark_mode;
                    let mono =
                        egui::FontId::monospace(ui.text_style_height(&egui::TextStyle::Body));
                    let mut where_layouter =
                        |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
                            let job = sql_highlighter::sql_layout_job(
                                text.as_str(),
                                mono.clone(),
                                dark_mode,
                            );
                            ui.fonts_mut(|f| f.layout_job(job))
                        };
                    let where_out = egui::TextEdit::singleline(&mut self.where_clause)
                        .id(where_id)
                        .hint_text("e.g. id > 10")
                        .desired_width(where_w)
                        .frame(egui::Frame::NONE)
                        .layouter(&mut where_layouter)
                        .show(ui);

                    // Auto-pair for WHERE field.
                    if where_out.response.has_focus() {
                        let events: Vec<egui::Event> = ui.input(|i| i.events.clone());
                        let wc = where_out.cursor_range.map(|cr| cr.primary.index);
                        sql_autocomplete::handle_auto_pair(&mut self.where_clause, wc, &events);
                    }

                    let where_lost_focus = where_out.response.lost_focus();
                    self.handle_filter_autocomplete(
                        ui, &where_out, where_id, true, // is_where
                    );

                    ui.separator();

                    // ORDER BY section
                    ui.label(egui::RichText::new("ORDER BY").color(hint_color).small());

                    let order_id = egui::Id::new("filter_order_text");
                    sql_autocomplete::consume_autocomplete_keys(ui, &mut self.order_autocomplete);
                    let mut order_layouter =
                        |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap: f32| {
                            let job = sql_highlighter::sql_layout_job(
                                text.as_str(),
                                mono.clone(),
                                dark_mode,
                            );
                            ui.fonts_mut(|f| f.layout_job(job))
                        };
                    // Reserve space on the right for the export icon (~24px)
                    let order_w = (ui.available_width() - 28.0).max(60.0);
                    let order_out = egui::TextEdit::singleline(&mut self.order_clause)
                        .id(order_id)
                        .hint_text("e.g. id DESC")
                        .desired_width(order_w)
                        .frame(egui::Frame::NONE)
                        .layouter(&mut order_layouter)
                        .show(ui);

                    // Auto-pair for ORDER BY field.
                    if order_out.response.has_focus() {
                        let events: Vec<egui::Event> = ui.input(|i| i.events.clone());
                        let oc = order_out.cursor_range.map(|cr| cr.primary.index);
                        sql_autocomplete::handle_auto_pair(&mut self.order_clause, oc, &events);
                    }

                    let order_lost_focus = order_out.response.lost_focus();
                    self.handle_filter_autocomplete(
                        ui, &order_out, order_id, false, // is_where
                    );

                    // Reload on Enter (only if autocomplete is NOT open)
                    let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if enter_pressed
                        && (where_lost_focus || order_lost_focus)
                        && !self.where_autocomplete.open
                        && !self.order_autocomplete.open
                    {
                        // Sync ORDER BY text → SortState before reload
                        self.sort_state = SortState::from_order_clause(&self.order_clause);
                        self.page = 0;
                        self.load(tab_id, cmd_tx);
                    }

                    // Export icon — far right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.render_export_icon(ui, hint_color, 16.0);
                    });
                });
            });
    }

    fn handle_filter_autocomplete(
        &mut self,
        ui: &mut egui::Ui,
        te_output: &TextEditOutput,
        text_edit_id: egui::Id,
        is_where: bool,
    ) {
        let state = if is_where {
            &mut self.where_autocomplete
        } else {
            &mut self.order_autocomplete
        };
        let text = if is_where {
            &self.where_clause
        } else {
            &self.order_clause
        };

        let has_focus = te_output.response.has_focus();
        if !has_focus {
            state.close();
            return;
        }

        let cursor_char_pos = te_output.cursor_range.map(|cr| cr.primary.index);
        if let Some(pos) = cursor_char_pos {
            // Collect column names from current result for suggestions.
            let column_names: Vec<String> = self
                .result
                .as_ref()
                .map(|r| r.columns.iter().map(|c| c.name.clone()).collect())
                .unwrap_or_default();
            sql_autocomplete::update_autocomplete(state, text, pos, &column_names);
        }

        // Compute cursor screen position for popup anchoring.
        let cursor_screen_pos = cursor_char_pos.map(|char_pos| {
            let ccursor = egui::text::CCursor::new(char_pos);
            let galley_offset = te_output.galley.pos_from_cursor(ccursor);
            let line_height = ui.text_style_height(&egui::TextStyle::Body);
            egui::pos2(
                te_output.galley_pos.x + galley_offset.min.x,
                te_output.galley_pos.y + galley_offset.min.y + line_height,
            )
        });

        if let Some(accepted) =
            sql_autocomplete::show_autocomplete_popup(ui, state, text_edit_id, cursor_screen_pos)
        {
            let text_mut = if is_where {
                &mut self.where_clause
            } else {
                &mut self.order_clause
            };
            let new_cursor = sql_autocomplete::apply_suggestion(
                text_mut,
                accepted.prefix_char_start,
                accepted.prefix_char_len,
                &accepted.replacement,
            );
            if let Some(mut te_state) = egui::TextEdit::load_state(ui.ctx(), te_output.response.id)
            {
                let cc = egui::text::CCursor::new(new_cursor);
                te_state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::one(cc)));
                te_state.store(ui.ctx(), te_output.response.id);
            }
            te_output.response.request_focus();
        }
    }
}
