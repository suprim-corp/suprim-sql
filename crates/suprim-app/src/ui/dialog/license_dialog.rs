//! Account dialog — sign-in form (not logged in) or account info (logged in).

use eframe::egui;

use suprim_core::constants::WEB_URL;

// ── Result ──────────────────────────────────────────────────────────────────

/// Result returned from the account dialog each frame.
pub enum LicenseDialogResult {
    /// Dialog still open.
    Pending,
    /// User clicked "Sign In".
    SignIn { email: String, password: String },
    /// User clicked "Sign Out".
    SignOut,
    /// User closed / cancelled.
    Cancelled,
}

// ── State ───────────────────────────────────────────────────────────────────

/// State for the account dialog.
pub struct LicenseDialog {
    email: String,
    password: String,
    error: Option<String>,
    tier_name: String,
    /// Signed-in email (shown in account view).
    current_email: Option<String>,
}

impl LicenseDialog {
    /// Create dialog for sign-in (not logged in).
    pub fn new(tier_name: &str) -> Self {
        Self {
            email: String::new(),
            password: String::new(),
            error: None,
            tier_name: tier_name.to_string(),
            current_email: None,
        }
    }

    /// Create dialog showing account info (logged in).
    pub fn with_info(tier_name: &str, email: Option<&str>) -> Self {
        Self {
            email: String::new(),
            password: String::new(),
            error: None,
            tier_name: tier_name.to_string(),
            current_email: email.map(|s| s.to_string()),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> LicenseDialogResult {
        let mut result = LicenseDialogResult::Pending;
        let mut is_open = true;
        let is_signed_in = self.current_email.is_some();

        let title = if is_signed_in { "Account" } else { "Sign In" };

        #[allow(unused_mut)]
        let mut window = egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .fixed_size([440.0, 340.0]);

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
            #[cfg(target_os = "macos")]
            self.render_macos_title_bar(ui, title, &mut is_open);

            ui.add_space(8.0);

            if is_signed_in {
                self.render_account_view(ui, &mut result, &mut is_open);
            } else {
                self.render_sign_in_view(ui, &mut result, &mut is_open);
            }
        });

        #[cfg(not(target_os = "macos"))]
        if !title_bar_open {
            is_open = false;
        }

        if !is_open {
            return LicenseDialogResult::Cancelled;
        }

        result
    }

    // ── macOS title bar ─────────────────────────────────────────────────

    #[cfg(target_os = "macos")]
    fn render_macos_title_bar(&self, ui: &mut egui::Ui, title: &str, is_open: &mut bool) {
        ui.horizontal(|ui| {
            let radius = 6.0;
            let (dot_rect, resp) = ui
                .allocate_exact_size(egui::vec2(radius * 2.0, radius * 2.0), egui::Sense::click());
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
                *is_open = false;
            }
            let remaining = ui.available_width();
            let title_w = title.len() as f32 * 8.5;
            ui.add_space((remaining - title_w).max(0.0) / 2.0);
            ui.label(egui::RichText::new(title).size(15.0).weak());
        });
        ui.separator();
    }

    // ── Account view (signed in) ────────────────────────────────────────

    fn render_account_view(
        &self,
        ui: &mut egui::Ui,
        result: &mut LicenseDialogResult,
        is_open: &mut bool,
    ) {
        // Tier badge
        let (tier_icon, tier_label, tier_color) = if self.tier_name == "Premium" {
            (
                egui_phosphor::regular::CROWN,
                "Premium",
                egui::Color32::from_rgb(255, 180, 50),
            )
        } else {
            (
                egui_phosphor::regular::USER,
                "Free",
                ui.visuals().weak_text_color(),
            )
        };

        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{tier_icon}  {tier_label}"))
                    .color(tier_color)
                    .strong()
                    .size(18.0),
            );
        });

        ui.add_space(16.0);

        // Account info
        egui::Grid::new("account_info")
            .num_columns(2)
            .spacing([12.0, 10.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Email:").weak().size(14.0));
                ui.label(
                    egui::RichText::new(self.current_email.as_deref().unwrap_or("—")).size(14.0),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Plan:").weak().size(14.0));
                ui.label(
                    egui::RichText::new(&self.tier_name)
                        .color(tier_color)
                        .strong()
                        .size(14.0),
                );
                ui.end_row();
            });

        ui.add_space(16.0);

        // Premium features or upgrade CTA
        if self.tier_name == "Premium" {
            ui.label(egui::RichText::new("Included:").weak().size(13.0));
            ui.add_space(6.0);
            for feature in [
                "Unlimited connections",
                "MongoDB & MSSQL drivers",
                "Structure Synchronization",
                "iCloud Keychain sync",
            ] {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::CHECK)
                            .color(egui::Color32::from_rgb(76, 175, 80))
                            .size(14.0),
                    );
                    ui.label(egui::RichText::new(feature).size(14.0));
                });
            }
        } else {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "{}  Upgrade to Premium for unlimited connections, MongoDB, MSSQL, and more.",
                        egui_phosphor::regular::ARROW_UP
                    ))
                    .weak()
                    .size(13.0),
                );
            });
            ui.add_space(6.0);
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{}  Upgrade", egui_phosphor::regular::CROWN))
                            .size(14.0)
                            .color(egui::Color32::from_rgb(100, 60, 0)),
                    )
                    .fill(egui::Color32::from_rgb(255, 200, 80)),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                ui.ctx()
                    .open_url(egui::OpenUrl::new_tab(format!("{WEB_URL}/pricing")));
            }
        }

        // Footer
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!(
                            "{}  Sign Out",
                            egui_phosphor::regular::SIGN_OUT
                        ))
                        .size(14.0)
                        .color(egui::Color32::from_rgb(220, 60, 60)),
                    )
                    .frame(false),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *result = LicenseDialogResult::SignOut;
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(egui::RichText::new("Close").size(14.0))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    *is_open = false;
                }
            });
        });
    }

    // ── Sign In view (not logged in) ────────────────────────────────────

    fn render_sign_in_view(
        &mut self,
        ui: &mut egui::Ui,
        result: &mut LicenseDialogResult,
        is_open: &mut bool,
    ) {
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new("Sign in to your SuprimSQL account").size(16.0));
        });

        ui.add_space(20.0);

        // Form
        let field_width = 300.0;
        egui::Grid::new("sign_in_form")
            .num_columns(2)
            .spacing([12.0, 14.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Email:").size(14.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.email)
                        .desired_width(field_width)
                        .font(egui::TextStyle::Body)
                        .hint_text("your@email.com"),
                );
                ui.end_row();

                ui.label(egui::RichText::new("Password:").size(14.0));
                ui.add(
                    egui::TextEdit::singleline(&mut self.password)
                        .desired_width(field_width)
                        .font(egui::TextStyle::Body)
                        .hint_text("••••••••")
                        .password(true),
                );
                ui.end_row();
            });

        ui.add_space(6.0);

        // Links row
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Label::new(
                        egui::RichText::new("Forgot password?")
                            .size(13.0)
                            .color(egui::Color32::from_rgb(100, 160, 255)),
                    )
                    .sense(egui::Sense::click()),
                )
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                ui.ctx()
                    .open_url(egui::OpenUrl::new_tab(format!("{WEB_URL}/forgot-password")));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        egui::Label::new(
                            egui::RichText::new("Create account")
                                .size(13.0)
                                .color(egui::Color32::from_rgb(100, 160, 255)),
                        )
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    ui.ctx()
                        .open_url(egui::OpenUrl::new_tab(format!("{WEB_URL}/signup")));
                }
            });
        });

        ui.add_space(6.0);

        // Error message
        if let Some(err) = &self.error {
            ui.label(
                egui::RichText::new(err)
                    .color(egui::Color32::from_rgb(220, 60, 60))
                    .size(13.0),
            );
            ui.add_space(4.0);
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        // Buttons
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let can_sign_in = !self.email.trim().is_empty() && !self.password.trim().is_empty();

                if ui
                    .add_enabled(
                        can_sign_in,
                        egui::Button::new(
                            egui::RichText::new(format!(
                                "{}  Sign In",
                                egui_phosphor::regular::SIGN_IN
                            ))
                            .size(14.0)
                            .color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(59, 130, 246)),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    *result = LicenseDialogResult::SignIn {
                        email: self.email.trim().to_string(),
                        password: self.password.trim().to_string(),
                    };
                }

                if ui
                    .button(egui::RichText::new("Cancel").size(14.0))
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    *is_open = false;
                }
            });
        });
    }
}
