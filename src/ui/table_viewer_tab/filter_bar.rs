/// Filter bar — WHERE / ORDER BY input fields with reload button.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::TableViewerTab;

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

                    // Reload button
                    if self.is_loading {
                        ui.spinner();
                    } else {
                        let resp = ui.add(
                            egui::Label::new(
                                egui::RichText::new(egui_phosphor::regular::ARROW_CLOCKWISE)
                                    .color(hint_color)
                                    .size(16.0),
                            )
                            .selectable(false)
                            .sense(egui::Sense::click()),
                        );
                        if resp.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                        if resp.clicked() {
                            self.page = 0;
                            self.load(tab_id, cmd_tx);
                        }
                    }

                    ui.separator();

                    // WHERE section
                    let remaining = ui.available_width();
                    let where_w = (remaining * 0.55 - 50.0).max(80.0);
                    ui.label(egui::RichText::new("WHERE").color(hint_color).small());
                    let where_edit = egui::TextEdit::singleline(&mut self.where_clause)
                        .hint_text("e.g. id > 10")
                        .desired_width(where_w)
                        .frame(egui::Frame::NONE);
                    let where_resp = ui.add(where_edit);

                    ui.separator();

                    // ORDER BY section
                    ui.label(egui::RichText::new("ORDER BY").color(hint_color).small());
                    let order_edit = egui::TextEdit::singleline(&mut self.order_clause)
                        .hint_text("e.g. id DESC")
                        .desired_width(ui.available_width())
                        .frame(egui::Frame::NONE);
                    let order_resp = ui.add(order_edit);

                    // Reload on Enter
                    let enter =
                        where_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let enter2 =
                        order_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if enter || enter2 {
                        self.page = 0;
                        self.load(tab_id, cmd_tx);
                    }
                });
            });
    }
}
