/// Filter bar — WHERE / ORDER BY input fields with reload button and autocomplete.
use eframe::egui;
use egui::text_edit::TextEditOutput;
use suprim_sql::db::driver::DbCommand;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::new_row_editor::NewRowEditor;
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

                    // Reload / spinner
                    if self.is_loading {
                        ui.spinner();
                    } else {
                        let reload_resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(
                                    egui_phosphor::regular::ARROW_COUNTER_CLOCKWISE,
                                )
                                .color(icon_color)
                                .size(icon_size),
                            )
                            .selectable(false)
                            .sense(egui::Sense::click()),
                        );
                        if reload_resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if reload_resp.clicked() {
                            self.page = 0;
                            self.load(tab_id, cmd_tx);
                        }
                        reload_resp.on_hover_text("Reload Data");
                    }

                    // Add Row (+)
                    let add_resp = ui.add(
                        egui::Label::new(
                            egui::RichText::new(egui_phosphor::regular::PLUS)
                                .color(icon_color)
                                .size(icon_size),
                        )
                        .selectable(false)
                        .sense(egui::Sense::click()),
                    );
                    if add_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if add_resp.clicked() {
                        if let Some(result) = &self.result {
                            self.new_row_editor = Some(NewRowEditor::new(result.columns.clone()));
                        }
                    }
                    add_resp.on_hover_text("Add Row");

                    // Delete Row (−)
                    let has_selection = self.selected_cell.is_some() || self.selected_row.is_some();
                    let del_resp = ui.add(
                        egui::Label::new(
                            egui::RichText::new(egui_phosphor::regular::MINUS)
                                .color(if has_selection {
                                    icon_color
                                } else {
                                    ui.visuals().noninteractive().bg_stroke.color
                                })
                                .size(icon_size),
                        )
                        .selectable(false)
                        .sense(if has_selection {
                            egui::Sense::click()
                        } else {
                            egui::Sense::hover()
                        }),
                    );
                    if has_selection && del_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if has_selection && del_resp.clicked() {
                        self.pending_toolbar_delete = true;
                    }
                    del_resp.on_hover_text(if has_selection {
                        "Delete Selected Row"
                    } else {
                        "Delete Row (select a cell first)"
                    });

                    // Undo (↶) — revert last pending change
                    let has_undo = !self.pending.undo_stack.is_empty();
                    let undo_resp = ui.add(
                        egui::Label::new(
                            egui::RichText::new(egui_phosphor::regular::ARROW_U_UP_LEFT)
                                .color(if has_undo {
                                    icon_color
                                } else {
                                    ui.visuals().noninteractive().bg_stroke.color
                                })
                                .size(icon_size),
                        )
                        .selectable(false)
                        .sense(if has_undo {
                            egui::Sense::click()
                        } else {
                            egui::Sense::hover()
                        }),
                    );
                    if has_undo && undo_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if has_undo && undo_resp.clicked() {
                        self.pending_undo = true;
                    }
                    undo_resp.on_hover_text(if has_undo {
                        "Undo Last Edit"
                    } else {
                        "Undo (no edits to undo)"
                    });

                    // Execute (▲) — commit all pending changes to database
                    let has_pending = self.pending.has_changes();
                    let change_count = self.pending.change_count();
                    let exec_resp = ui.add(
                        egui::Label::new(
                            egui::RichText::new(egui_phosphor::regular::ARROW_FAT_UP)
                                .color(if has_pending {
                                    egui::Color32::from_rgb(100, 200, 100)
                                } else {
                                    icon_color
                                })
                                .size(icon_size),
                        )
                        .selectable(false)
                        .sense(egui::Sense::click()),
                    );
                    if exec_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if exec_resp.clicked() {
                        if has_pending {
                            self.pending_execute = true;
                        } else {
                            // No pending changes: just reload
                            self.page = 0;
                            self.load(tab_id, cmd_tx);
                        }
                    }
                    exec_resp.on_hover_text(if has_pending {
                        format!("Execute {} pending change(s)", change_count)
                    } else {
                        "Execute / Apply Filters".to_string()
                    });

                    // Pending change count badge
                    if has_pending {
                        ui.label(
                            egui::RichText::new(format!("{}", change_count))
                                .color(egui::Color32::from_rgb(100, 200, 100))
                                .small()
                                .strong(),
                        );
                    }

                    ui.separator();

                    // WHERE section
                    let remaining = ui.available_width();
                    let where_w = (remaining * 0.55 - 50.0).max(80.0);
                    ui.label(egui::RichText::new("WHERE").color(hint_color).small());

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
                    let order_out = egui::TextEdit::singleline(&mut self.order_clause)
                        .id(order_id)
                        .hint_text("e.g. id DESC")
                        .desired_width(ui.available_width())
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
                        self.page = 0;
                        self.load(tab_id, cmd_tx);
                    }
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
