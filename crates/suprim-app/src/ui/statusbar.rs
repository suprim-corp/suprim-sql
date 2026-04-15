use eframe::egui;

/// Bottom status bar.
pub struct StatusBar;

impl StatusBar {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&self, ui: &mut egui::Ui, status: &str, tier_name: &str) {
        ui.horizontal(|ui| {
            ui.label(status);

            // Push tier badge to the right
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

                egui::Frame::new()
                    .fill(bg_color)
                    .inner_margin(egui::Margin::symmetric(8, 2))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        let text = egui::RichText::new(format!("{icon} {label}")).color(text_color);
                        let resp = ui.label(text);
                        if tier_name == "Free" {
                            resp.on_hover_text(
                                "Free plan — 5 connections max. Upgrade to Premium for unlimited.",
                            );
                        }
                    });
            });
        });
    }
}
