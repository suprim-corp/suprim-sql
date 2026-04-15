use eframe::egui;

/// Bottom status bar.
pub struct StatusBar;

impl StatusBar {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&self, ui: &mut egui::Ui, status: &str, tier_name: &str) {
        let bar_h = ui.available_height();

        ui.horizontal_centered(|ui| {
            ui.label(status);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (icon, label, text_color, bg_color) = match tier_name {
                    "Premium" => (
                        egui_phosphor::regular::CROWN,
                        "Premium",
                        egui::Color32::from_rgb(100, 60, 0),
                        egui::Color32::from_rgb(255, 200, 80),
                    ),
                    _ => (
                        egui_phosphor::regular::LOCK_SIMPLE,
                        "Free",
                        ui.visuals().weak_text_color(),
                        if ui.visuals().dark_mode {
                            egui::Color32::from_rgba_premultiplied(255, 255, 255, 15)
                        } else {
                            egui::Color32::from_rgba_premultiplied(0, 0, 0, 12)
                        },
                    ),
                };

                let badge_text = format!("{icon} {label}");
                let font_id = egui::FontId::proportional(12.0);
                let galley = ui.painter().layout_no_wrap(badge_text, font_id, text_color);
                let pad_h: f32 = 8.0;
                let pad_v: f32 = 2.0;
                let badge_w = galley.size().x + pad_h * 2.0;
                let badge_h = galley.size().y + pad_v * 2.0;

                // Allocate full bar height so rect vertical center = bar center
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(badge_w, bar_h), egui::Sense::hover());

                // Pill centered vertically in the bar
                let center_y = rect.min.y + bar_h / 2.0;
                let pill = egui::Rect::from_center_size(
                    egui::pos2(rect.center().x, center_y),
                    egui::vec2(badge_w, badge_h),
                );

                ui.painter().rect_filled(pill, 4.0, bg_color);
                ui.painter().galley(
                    egui::pos2(pill.left() + pad_h, pill.top() + pad_v),
                    galley,
                    text_color,
                );

                if tier_name == "Free" {
                    resp.on_hover_text(
                        "Free plan — 5 connections max. Upgrade to Premium for unlimited.",
                    );
                }
            });
        });
    }
}
