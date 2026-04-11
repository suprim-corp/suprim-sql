//! Reusable input dialog — prompts the user for a single text value.

use eframe::egui;

/// Result returned from the input dialog each frame.
pub enum InputDialogResult {
    /// Dialog is still open, user has not decided yet.
    Pending,
    /// User confirmed with the entered value.
    Confirmed(String),
    /// User cancelled.
    Cancelled,
}

/// What kind of input prompt this dialog represents.
#[derive(Clone)]
pub enum InputDialogKind {
    /// CREATE DATABASE on connection `conn_id`.
    NewDatabase { conn_id: uuid::Uuid },
    /// CREATE SCHEMA in database `database` on connection `conn_id`.
    NewSchema {
        conn_id: uuid::Uuid,
        database: String,
    },
}

/// State for the input dialog.
pub struct InputDialog {
    pub title: String,
    pub label: String,
    pub value: String,
    pub kind: InputDialogKind,
}

impl InputDialog {
    /// Create a "New Database" input dialog.
    pub fn new_database(conn_id: uuid::Uuid) -> Self {
        Self {
            title: "New Database".to_string(),
            label: "Database name".to_string(),
            value: String::new(),
            kind: InputDialogKind::NewDatabase { conn_id },
        }
    }

    /// Create a "New Schema" input dialog.
    pub fn new_schema(conn_id: uuid::Uuid, database: String) -> Self {
        Self {
            title: "New Schema".to_string(),
            label: "Schema name".to_string(),
            value: String::new(),
            kind: InputDialogKind::NewSchema { conn_id, database },
        }
    }

    /// Render the dialog. Returns the user's choice each frame.
    pub fn show(&mut self, ctx: &egui::Context) -> InputDialogResult {
        let mut result = InputDialogResult::Pending;

        egui::Window::new(&self.title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .min_width(340.0)
            .show(ctx, |ui| {
                ui.add_space(8.0);
                ui.label(&self.label);
                ui.add_space(4.0);

                let text_edit = ui.add(
                    egui::TextEdit::singleline(&mut self.value)
                        .desired_width(300.0)
                        .hint_text("Enter name..."),
                );

                // Auto-focus the text field on first frame
                if text_edit.gained_focus() || self.value.is_empty() {
                    text_edit.request_focus();
                }

                // Enter key confirms
                if text_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let trimmed = self.value.trim().to_string();
                    if !trimmed.is_empty() {
                        result = InputDialogResult::Confirmed(trimmed);
                    }
                }

                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    let can_confirm = !self.value.trim().is_empty();
                    if ui
                        .add_enabled(can_confirm, egui::Button::new("Create"))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        result = InputDialogResult::Confirmed(self.value.trim().to_string());
                    }
                    if ui
                        .button("Cancel")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        result = InputDialogResult::Cancelled;
                    }
                });
            });

        result
    }
}
