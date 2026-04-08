//! "About SuprimSQL" modal dialog.

use eframe::egui;

/// Show the About dialog centered on screen.
/// Returns `true` if the dialog should remain open, `false` when closed.
pub fn show_about_dialog(ctx: &egui::Context) -> bool {
    let mut open = true;

    egui::Window::new("About SuprimSQL")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([300.0, 160.0])
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(12.0);
                ui.heading("SuprimSQL");
                ui.add_space(4.0);
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.add_space(4.0);
                ui.label("A modern SQL client for PostgreSQL");
                ui.add_space(12.0);
                if ui
                    .button("Close")
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    open = false;
                }
            });
        });

    open
}
