//! Upgrade prompt dialog — shown when a user hits a premium-only limitation.

use eframe::egui;

/// Result returned from the upgrade prompt each frame.
pub enum UpgradePromptResult {
    /// Dialog still open.
    Pending,
    /// User clicked "Enter License Key".
    OpenLicenseDialog,
    /// User dismissed the dialog.
    Dismissed,
}

/// State for the upgrade prompt dialog.
pub struct UpgradePrompt {
    pub message: String,
}

impl UpgradePrompt {
    pub fn new(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }

    pub fn show(&self, ctx: &egui::Context) -> UpgradePromptResult {
        let mut result = UpgradePromptResult::Pending;
        let mut is_open = true;

        #[allow(unused_mut)]
        let mut window = egui::Window::new("Upgrade to Premium")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([380.0, 200.0]);

        #[cfg(target_os = "macos")]
        {
            window = window.title_bar(false);
        }
        #[cfg(not(target_os = "macos"))]
        let mut title_bar_open = true;
        #[cfg(not(target_os = "macos"))]
        {
            window = window.open(&mut title_bar_open);
        }

        window.show(ctx, |ui| {
            // macOS: custom title bar
            #[cfg(target_os = "macos")]
            {
                ui.horizontal(|ui| {
                    let radius = 6.0;
                    let (dot_rect, resp) = ui.allocate_exact_size(
                        egui::vec2(radius * 2.0, radius * 2.0),
                        egui::Sense::click(),
                    );
                    let center = dot_rect.center();
                    let color = if resp.hovered() {
                        egui::Color32::from_rgb(255, 80, 80)
                    } else {
                        egui::Color32::from_rgb(255, 59, 48)
                    };
                    ui.painter().circle_filled(center, radius, color);
                    if resp.hovered() {
                        ui.painter().text(
                            center,
                            egui::Align2::CENTER_CENTER,
                            egui_phosphor::regular::X,
                            egui::FontId::proportional(8.0),
                            egui::Color32::from_rgb(80, 0, 0),
                        );
                    }
                    if resp.clicked() {
                        is_open = false;
                    }
                    let remaining = ui.available_width();
                    ui.add_space((remaining - 140.0).max(0.0) / 2.0);
                    ui.label(egui::RichText::new("Upgrade to Premium").size(13.0).weak());
                });
                ui.separator();
            }

            ui.add_space(16.0);

            // Crown icon + message
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::CROWN)
                        .size(32.0)
                        .color(egui::Color32::from_rgb(255, 180, 50)),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(&self.message)
                        .size(13.0)
                        .color(ui.visuals().text_color()),
                );
            });

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(4.0);

            // Buttons
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(format!(
                                    "{} Enter License Key",
                                    egui_phosphor::regular::KEY
                                ))
                                .color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(59, 130, 246)),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        result = UpgradePromptResult::OpenLicenseDialog;
                    }

                    if ui
                        .button("Later")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        is_open = false;
                    }
                });
            });
        });

        #[cfg(not(target_os = "macos"))]
        if !title_bar_open {
            is_open = false;
        }

        if !is_open {
            return UpgradePromptResult::Dismissed;
        }

        result
    }
}
