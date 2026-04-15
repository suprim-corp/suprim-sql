//! License activation dialog — enter license key to activate Premium.

use eframe::egui;

/// Result returned from the license dialog each frame.
pub enum LicenseDialogResult {
    /// Dialog is still open, user has not decided yet.
    Pending,
    /// User clicked "Activate" with the entered license key.
    Activate { key: String, email: String },
    /// User cancelled.
    Cancelled,
}

/// State for the license activation dialog.
pub struct LicenseDialog {
    pub license_key: String,
    pub email: String,
    pub error: Option<String>,
    pub tier_name: String,
}

impl LicenseDialog {
    pub fn new(tier_name: &str) -> Self {
        Self {
            license_key: String::new(),
            email: String::new(),
            error: None,
            tier_name: tier_name.to_string(),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> LicenseDialogResult {
        let mut result = LicenseDialogResult::Pending;
        let mut is_open = true;

        #[allow(unused_mut)]
        let mut window = egui::Window::new("License Activation")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([400.0, 280.0]);

        #[cfg(target_os = "macos")]
        {
            window = window.title_bar(false);
        }
        #[cfg(not(target_os = "macos"))]
        {
            window = window.open(&mut is_open);
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
                    ui.label(egui::RichText::new("License Activation").size(13.0).weak());
                });
                ui.separator();
            }

            ui.add_space(8.0);

            // Current tier
            let tier_color = if self.tier_name == "Premium" {
                egui::Color32::from_rgb(255, 180, 50)
            } else {
                ui.visuals().weak_text_color()
            };
            ui.horizontal(|ui| {
                ui.label("Current plan:");
                ui.label(
                    egui::RichText::new(&self.tier_name)
                        .color(tier_color)
                        .strong(),
                );
            });

            ui.add_space(12.0);

            // Form
            egui::Grid::new("license_form")
                .num_columns(2)
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Email:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.email)
                            .desired_width(280.0)
                            .hint_text("your@email.com"),
                    );
                    ui.end_row();

                    ui.label("License Key:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.license_key)
                            .desired_width(280.0)
                            .hint_text("XXXX-XXXX-XXXX-XXXX")
                            .password(true),
                    );
                    ui.end_row();
                });

            ui.add_space(8.0);

            // Error message
            if let Some(err) = &self.error {
                ui.label(
                    egui::RichText::new(err)
                        .color(egui::Color32::from_rgb(220, 60, 60))
                        .size(11.0),
                );
                ui.add_space(4.0);
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Buttons
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let can_activate =
                        !self.license_key.trim().is_empty() && !self.email.trim().is_empty();

                    if ui
                        .add_enabled(
                            can_activate,
                            egui::Button::new(
                                egui::RichText::new(format!(
                                    "{} Activate",
                                    egui_phosphor::regular::KEY
                                ))
                                .color(egui::Color32::WHITE),
                            )
                            .fill(egui::Color32::from_rgb(59, 130, 246)),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        result = LicenseDialogResult::Activate {
                            key: self.license_key.trim().to_string(),
                            email: self.email.trim().to_string(),
                        };
                    }

                    if ui
                        .button("Cancel")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        is_open = false;
                    }
                });
            });
        });

        if !is_open {
            return LicenseDialogResult::Cancelled;
        }

        result
    }
}
