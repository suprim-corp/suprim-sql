/// Connection dialog — modal UI for creating or editing a database connection.
/// Config building and validation logic is in `connection_dialog_config.rs`.
use eframe::egui;
use suprim_sql::db::connection::ConnectionConfig;
use uuid::Uuid;

use super::connection_dialog_config::{build_config, extract_fields, DbType, DialogFields};

/// Result returned from a dialog each frame.
pub enum DialogResult {
    Pending,
    Cancelled,
    Confirmed(ConnectionConfig),
}

/// Modal dialog for creating or editing a database connection.
pub struct ConnectionDialog {
    /// When editing an existing connection, this holds the original id.
    edit_id: Option<Uuid>,

    name: String,
    db_type: DbType,

    // Generic fields
    host: String,
    port: String,
    database: String,
    username: String,
    password: String,

    // SQLite-specific
    sqlite_path: String,

    // MongoDB-specific
    mongodb_uri: String,

    error: Option<String>,
}

impl ConnectionDialog {
    /// Open dialog for a brand-new connection.
    pub fn new() -> Self {
        Self {
            edit_id: None,
            name: String::new(),
            db_type: DbType::Postgres,
            host: "localhost".to_string(),
            port: "5432".to_string(),
            database: String::new(),
            username: String::new(),
            password: String::new(),
            sqlite_path: String::new(),
            mongodb_uri: "mongodb://localhost:27017".to_string(),
            error: None,
        }
    }

    /// Open dialog pre-populated with an existing connection for editing.
    pub fn from_config(config: &ConnectionConfig) -> Self {
        let f = extract_fields(config);
        Self {
            edit_id: Some(config.id),
            name: config.name.clone(),
            db_type: f.db_type,
            host: f.host,
            port: f.port,
            database: f.database,
            username: f.username,
            password: f.password,
            sqlite_path: f.sqlite_path,
            mongodb_uri: f.mongodb_uri,
            error: None,
        }
    }

    fn fields(&self) -> DialogFields<'_> {
        DialogFields {
            edit_id: self.edit_id,
            name: &self.name,
            db_type: &self.db_type,
            host: &self.host,
            port: &self.port,
            database: &self.database,
            username: &self.username,
            password: &self.password,
            sqlite_path: &self.sqlite_path,
            mongodb_uri: &self.mongodb_uri,
        }
    }

    /// Render the dialog. Returns `DialogResult` each frame.
    pub fn show(&mut self, ctx: &egui::Context) -> DialogResult {
        let mut result = DialogResult::Pending;

        let title = if self.edit_id.is_some() {
            "Edit Connection"
        } else {
            "New Connection"
        };
        let confirm_label = if self.edit_id.is_some() {
            "Save & Reconnect"
        } else {
            "Connect"
        };

        egui::Window::new(title)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .min_width(400.0)
            .show(ctx, |ui| {
                egui::Grid::new("conn_form")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.name);
                        ui.end_row();

                        ui.label("Type:");
                        let type_combo = egui::ComboBox::from_id_salt("db_type")
                            .selected_text(self.db_type.label())
                            .show_ui(ui, |ui| {
                                for db_type in DbType::all() {
                                    let selected = &self.db_type == db_type;
                                    if ui
                                        .selectable_label(selected, db_type.label())
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                        && !selected
                                    {
                                        self.db_type = db_type.clone();
                                        self.port = self.db_type.default_port().to_string();
                                    }
                                }
                            });
                        type_combo
                            .response
                            .on_hover_cursor(egui::CursorIcon::PointingHand);
                        ui.end_row();
                    });

                ui.separator();

                // Type-specific fields
                egui::Grid::new("conn_fields")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| match &self.db_type {
                        DbType::Sqlite => {
                            ui.label("File path:");
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.sqlite_path);
                                if ui
                                    .small_button("Browse\u{2026}")
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked()
                                {
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter("SQLite", &["db", "sqlite", "sqlite3"])
                                        .pick_file()
                                    {
                                        self.sqlite_path = path.to_string_lossy().to_string();
                                    }
                                }
                            });
                            ui.end_row();
                        }
                        DbType::MongoDB => {
                            ui.label("URI:");
                            ui.text_edit_singleline(&mut self.mongodb_uri);
                            ui.end_row();
                        }
                        _ => {
                            ui.label("Host:");
                            ui.text_edit_singleline(&mut self.host);
                            ui.end_row();

                            ui.label("Port:");
                            ui.text_edit_singleline(&mut self.port);
                            ui.end_row();

                            if !matches!(self.db_type, DbType::Redis) {
                                ui.label("Database:");
                                ui.text_edit_singleline(&mut self.database);
                                ui.end_row();

                                ui.label("Username:");
                                ui.text_edit_singleline(&mut self.username);
                                ui.end_row();

                                ui.label("Password:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.password).password(true),
                                );
                                ui.end_row();
                            }
                        }
                    });

                if let Some(err) = &self.error {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(err).color(egui::Color32::from_rgb(220, 80, 80)));
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(confirm_label)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        match build_config(&self.fields()) {
                            Ok(config) => {
                                result = DialogResult::Confirmed(config);
                                self.error = None;
                            }
                            Err(e) => self.error = Some(e),
                        }
                    }
                    if ui
                        .button("Cancel")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        result = DialogResult::Cancelled;
                    }
                });
            });

        result
    }
}
