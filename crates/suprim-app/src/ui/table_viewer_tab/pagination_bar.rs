/// Pagination bar — page navigation with prev/next buttons and row count.
use eframe::egui;
use suprim_core::db::commands::DbCommand;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::TableViewerTab;

impl TableViewerTab {
    pub(super) fn render_pagination_bar(
        &mut self,
        ui: &mut egui::Ui,
        tab_id: Uuid,
        cmd_tx: &mpsc::Sender<DbCommand>,
        hint_color: egui::Color32,
    ) {
        if self.result.is_none() {
            return;
        }

        let total_pages = self
            .total_count
            .map(|tc| ((tc as f64) / (self.page_size as f64)).ceil() as usize)
            .unwrap_or(0)
            .max(1);
        let current = self.page + 1;
        let is_last = current >= total_pages;

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Next
                let next = ui
                    .add_enabled(
                        !is_last,
                        egui::Button::new(egui_phosphor::regular::CARET_RIGHT).small(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if next.clicked() {
                    self.page += 1;
                    self.load(tab_id, cmd_tx);
                }

                // Page info
                let page_label = if let Some(tc) = self.total_count {
                    format!("{current} / {total_pages}  ({tc} rows)")
                } else {
                    format!("Page {current}")
                };
                ui.label(egui::RichText::new(page_label).color(hint_color).small());

                // Prev
                let prev = ui
                    .add_enabled(
                        self.page > 0,
                        egui::Button::new(egui_phosphor::regular::CARET_LEFT).small(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if prev.clicked() {
                    self.page -= 1;
                    self.load(tab_id, cmd_tx);
                }
            });
        });
    }
}
