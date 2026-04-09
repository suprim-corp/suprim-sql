//! Delete Connection confirmation dialog — simple modal with egui Window title bar.

use eframe::egui;

/// Result returned from the delete connection dialog each frame.
pub enum DeleteConnectionResult {
    /// Dialog is still open, user has not decided yet.
    Pending,
    /// User confirmed deletion.
    Confirmed,
    /// User cancelled.
    Cancelled,
}

/// State for the delete connection confirmation dialog.
pub struct DeleteConnectionDialog {
    pub title: String,
    pub message: String,
}

impl DeleteConnectionDialog {
    /// Create a new delete connection confirmation dialog.
    pub fn new(conn_name: &str) -> Self {
        Self {
            title: "Delete Connection".to_string(),
            message: format!(
                "Are you sure you want to delete \"{}\"?\nThis cannot be undone.",
                conn_name
            ),
        }
    }

    /// Render the dialog. Returns the user's choice each frame.
    pub fn show(&self, ctx: &egui::Context) -> DeleteConnectionResult {
        let mut result = DeleteConnectionResult::Pending;

        egui::Window::new(&self.title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(340.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(&self.message);
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    // Delete button (red/destructive)
                    let btn = egui::Button::new(
                        egui::RichText::new("Delete").color(egui::Color32::WHITE),
                    )
                    .fill(egui::Color32::from_rgb(200, 60, 60));
                    if ui
                        .add(btn)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        result = DeleteConnectionResult::Confirmed;
                    }

                    if ui
                        .button("Cancel")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        result = DeleteConnectionResult::Cancelled;
                    }
                });
            });

        result
    }
}
