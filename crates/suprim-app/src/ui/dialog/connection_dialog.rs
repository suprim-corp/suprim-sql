/// Connection dialog — modal UI for creating or editing a database connection.
/// Config building and validation logic is in `connection_dialog_config.rs`.
use eframe::egui;
use suprim_sql::db::commands::DbCommand;
use suprim_sql::db::connection::ConnectionConfig;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::connection_dialog_config::{build_config, extract_fields, DbType, DialogFields};

/// Result returned from a dialog each frame.
pub enum DialogResult {
    Pending,
    Cancelled,
    Confirmed(Box<ConnectionConfig>),
}

/// Test connection state.
#[derive(Clone)]
enum TestStatus {
    Idle,
    Testing,
    Success(String),
    Failed(String),
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

    // SSH tunnel fields
    ssh_enabled: bool,
    ssh_host: String,
    ssh_port: String,
    ssh_user: String,
    ssh_key_path: String,
    ssh_password: String,

    error: Option<String>,
    test_status: TestStatus,
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
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: "22".to_string(),
            ssh_user: String::new(),
            ssh_key_path: String::new(),
            ssh_password: String::new(),
            error: None,
            test_status: TestStatus::Idle,
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
            ssh_enabled: f.ssh_enabled,
            ssh_host: f.ssh_host,
            ssh_port: f.ssh_port,
            ssh_user: f.ssh_user,
            ssh_key_path: f.ssh_key_path,
            ssh_password: f.ssh_password,
            error: None,
            test_status: TestStatus::Idle,
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
            ssh_enabled: self.ssh_enabled,
            ssh_host: &self.ssh_host,
            ssh_port: &self.ssh_port,
            ssh_user: &self.ssh_user,
            ssh_key_path: &self.ssh_key_path,
            ssh_password: &self.ssh_password,
        }
    }

    /// Called when a TestConnectionResult event arrives.
    pub fn on_test_result(&mut self, success: bool, message: String) {
        self.test_status = if success {
            TestStatus::Success(message)
        } else {
            TestStatus::Failed(message)
        };
    }

    /// Render the dialog. Returns `DialogResult` each frame.
    pub fn show(&mut self, ctx: &egui::Context, cmd_tx: &mpsc::Sender<DbCommand>) -> DialogResult {
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

                // Type-specific fields + SSH tunnel (single grid for alignment)
                egui::Grid::new("conn_fields")
                    .num_columns(2)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        match &self.db_type {
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
                                        egui::TextEdit::singleline(&mut self.password)
                                            .password(true),
                                    );
                                    ui.end_row();
                                }
                            }
                        }

                        // SSH Tunnel fields (inside same grid for column alignment)
                        if !matches!(self.db_type, DbType::Sqlite) {
                            // Checkbox spans full row
                            ui.label("");
                            ui.checkbox(&mut self.ssh_enabled, "SSH Tunnel");
                            ui.end_row();

                            if self.ssh_enabled {
                                ui.label("SSH Host:");
                                ui.text_edit_singleline(&mut self.ssh_host);
                                ui.end_row();

                                ui.label("SSH Port:");
                                ui.text_edit_singleline(&mut self.ssh_port);
                                ui.end_row();

                                ui.label("SSH User:");
                                ui.text_edit_singleline(&mut self.ssh_user);
                                ui.end_row();

                                ui.label("Key File:");
                                ui.horizontal(|ui| {
                                    ui.text_edit_singleline(&mut self.ssh_key_path);
                                    if ui
                                        .small_button("Browse\u{2026}")
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        if let Some(path) = rfd::FileDialog::new()
                                            .set_directory(
                                                dirs_next::home_dir()
                                                    .map(|h| h.join(".ssh"))
                                                    .unwrap_or_default(),
                                            )
                                            .pick_file()
                                        {
                                            self.ssh_key_path = path.to_string_lossy().to_string();
                                        }
                                    }
                                });
                                ui.end_row();

                                ui.label("SSH Password:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.ssh_password)
                                        .password(true),
                                );
                                ui.end_row();
                            }
                        }
                    });

                if let Some(err) = &self.error {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(err).color(egui::Color32::from_rgb(220, 80, 80)));
                }

                // Show test connection result
                match &self.test_status {
                    TestStatus::Testing => {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(egui::RichText::new("Testing connection...").weak());
                        });
                    }
                    TestStatus::Success(msg) => {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("\u{2714} {msg}"))
                                .color(egui::Color32::from_rgb(80, 180, 80)),
                        );
                    }
                    TestStatus::Failed(msg) => {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("\u{2716} {msg}"))
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                    }
                    TestStatus::Idle => {}
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let is_testing = matches!(self.test_status, TestStatus::Testing);
                    if ui
                        .add_enabled(!is_testing, egui::Button::new("Test Connection"))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        match build_config(&self.fields()) {
                            Ok(config) => {
                                self.error = None;
                                self.test_status = TestStatus::Testing;
                                let _ = cmd_tx.try_send(DbCommand::TestConnection { config });
                            }
                            Err(e) => self.error = Some(e),
                        }
                    }

                    ui.add_space(8.0);

                    if ui
                        .button(confirm_label)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        match build_config(&self.fields()) {
                            Ok(config) => {
                                result = DialogResult::Confirmed(Box::new(config));
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
