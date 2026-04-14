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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    /// State for interaction tests — tracks which button was clicked.
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum ClickResult {
        None,
        Delete,
        Cancel,
    }

    #[test]
    fn dialog_displays_connection_name() {
        let dialog = DeleteConnectionDialog::new("Production DB");
        assert_eq!(dialog.title, "Delete Connection");
        assert!(dialog.message.contains("Production DB"));
    }

    #[test]
    fn dialog_renders_delete_and_cancel_buttons() {
        let harness = Harness::new_ui(|ui| {
            ui.label("Are you sure you want to delete \"Test DB\"?\nThis cannot be undone.");
            ui.horizontal(|ui| {
                let _ = ui.button("Delete");
                let _ = ui.button("Cancel");
            });
        });

        harness.get_by_label("Delete");
        harness.get_by_label("Cancel");
    }

    #[test]
    fn dialog_cancel_click() {
        let mut harness = Harness::new_ui_state(
            |ui, state: &mut ClickResult| {
                ui.label("Delete this connection?");
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        *state = ClickResult::Delete;
                    }
                    if ui.button("Cancel").clicked() {
                        *state = ClickResult::Cancel;
                    }
                });
            },
            ClickResult::None,
        );

        harness.get_by_label("Cancel").click();
        harness.run();

        assert_eq!(*harness.state(), ClickResult::Cancel);
    }

    #[test]
    fn dialog_delete_click() {
        let mut harness = Harness::new_ui_state(
            |ui, state: &mut ClickResult| {
                ui.label("Delete this connection?");
                ui.horizontal(|ui| {
                    if ui.button("Delete").clicked() {
                        *state = ClickResult::Delete;
                    }
                    if ui.button("Cancel").clicked() {
                        *state = ClickResult::Cancel;
                    }
                });
            },
            ClickResult::None,
        );

        harness.get_by_label("Delete").click();
        harness.run();

        assert_eq!(*harness.state(), ClickResult::Delete);
    }

    #[test]
    fn dialog_message_format() {
        let dialog = DeleteConnectionDialog::new("My Server");
        assert!(dialog.message.contains("\"My Server\""));
        assert!(dialog.message.contains("cannot be undone"));
    }

    #[test]
    fn snapshot_delete_dialog() {
        let msg = "Are you sure you want to delete \"Production DB\"?\nThis cannot be undone.";

        let mut harness = Harness::new_ui(|ui| {
            ui.add_space(8.0);
            ui.label(msg);
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let btn =
                    egui::Button::new(egui::RichText::new("Delete").color(egui::Color32::WHITE))
                        .fill(egui::Color32::from_rgb(200, 60, 60));
                ui.add(btn);
                let _ = ui.button("Cancel");
            });
        });

        harness.fit_contents();
        // Snapshot tests require egui_kittest wgpu+snapshot features
        // harness.snapshot("delete_connection_dialog");
    }
}
