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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::kittest::Queryable;
    use egui_kittest::Harness;

    #[derive(Clone, Debug, PartialEq)]
    enum ClickResult {
        None,
        Create(String),
        Cancel,
    }

    #[test]
    fn new_database_dialog_fields() {
        let conn_id = uuid::Uuid::new_v4();
        let dialog = InputDialog::new_database(conn_id);
        assert_eq!(dialog.title, "New Database");
        assert_eq!(dialog.label, "Database name");
        assert!(dialog.value.is_empty());
        assert!(matches!(dialog.kind, InputDialogKind::NewDatabase { .. }));
    }

    #[test]
    fn new_schema_dialog_fields() {
        let conn_id = uuid::Uuid::new_v4();
        let dialog = InputDialog::new_schema(conn_id, "mydb".to_string());
        assert_eq!(dialog.title, "New Schema");
        assert_eq!(dialog.label, "Schema name");
        assert!(dialog.value.is_empty());
        match &dialog.kind {
            InputDialogKind::NewSchema { database, .. } => {
                assert_eq!(database, "mydb");
            }
            _ => panic!("Expected NewSchema kind"),
        }
    }

    #[test]
    fn dialog_renders_create_and_cancel_buttons() {
        let harness = Harness::new_ui(|ui| {
            ui.label("Database name");
            ui.text_edit_singleline(&mut String::new());
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Button::new("Create"));
                ui.button("Cancel");
            });
        });

        harness.get_by_label("Cancel");
    }

    #[test]
    fn dialog_cancel_click() {
        let mut harness = Harness::new_ui_state(
            |ui, state: &mut ClickResult| {
                ui.label("Schema name");
                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() {
                        *state = ClickResult::Create("test".to_string());
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
    fn snapshot_input_dialog_new_database() {
        let mut harness = Harness::new_ui(|ui| {
            ui.add_space(8.0);
            ui.label("Database name");
            ui.add_space(4.0);
            let mut value = String::new();
            ui.add(
                egui::TextEdit::singleline(&mut value)
                    .desired_width(300.0)
                    .hint_text("Enter name..."),
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.add_enabled(false, egui::Button::new("Create"));
                ui.button("Cancel");
            });
        });

        harness.fit_contents();
        #[cfg(all(feature = "wgpu", feature = "snapshot"))]
        harness.snapshot("input_dialog_new_database");
    }
}
