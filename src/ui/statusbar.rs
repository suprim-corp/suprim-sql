use eframe::egui;

/// Bottom status bar.
pub struct StatusBar;

impl StatusBar {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&self, ui: &mut egui::Ui, status: &str) {
        ui.horizontal(|ui| {
            ui.label(status);
        });
    }
}
