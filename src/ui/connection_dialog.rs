use eframe::egui;
use suprim_sql::db::connection::{ConnectionConfig, DriverParams};
use uuid::Uuid;

/// Result returned from a dialog each frame.
pub enum DialogResult {
    Pending,
    Cancelled,
    Confirmed(ConnectionConfig),
}

/// Which database type is selected in the dialog.
#[derive(Debug, Clone, PartialEq)]
enum DbType {
    Sqlite,
    Postgres,
    Mysql,
    Redis,
    MongoDB,
    Mssql,
}

impl DbType {
    fn label(&self) -> &str {
        match self {
            DbType::Sqlite => "SQLite",
            DbType::Postgres => "PostgreSQL",
            DbType::Mysql => "MySQL / MariaDB",
            DbType::Redis => "Redis",
            DbType::MongoDB => "MongoDB",
            DbType::Mssql => "MSSQL / Azure",
        }
    }

    fn all() -> &'static [DbType] {
        &[
            DbType::Sqlite,
            DbType::Postgres,
            DbType::Mysql,
            DbType::Redis,
            DbType::MongoDB,
            DbType::Mssql,
        ]
    }
}

/// Modal dialog for creating or editing a database connection.
pub struct ConnectionDialog {
    /// When editing an existing connection, this holds the original id.
    /// None means "create new" (a fresh UUID will be assigned on confirm).
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
        let (db_type, host, port, database, username, password, sqlite_path, mongodb_uri) =
            match &config.params {
                DriverParams::Sqlite { path } => (
                    DbType::Sqlite,
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    path.to_string_lossy().to_string(),
                    String::new(),
                ),
                DriverParams::Postgres {
                    host,
                    port,
                    database,
                    user,
                    password_key,
                } => (
                    DbType::Postgres,
                    host.clone(),
                    port.to_string(),
                    database.clone(),
                    user.clone(),
                    password_key.clone(),
                    String::new(),
                    String::new(),
                ),
                DriverParams::Mysql {
                    host,
                    port,
                    database,
                    user,
                    password_key,
                } => (
                    DbType::Mysql,
                    host.clone(),
                    port.to_string(),
                    database.clone(),
                    user.clone(),
                    password_key.clone(),
                    String::new(),
                    String::new(),
                ),
                DriverParams::Redis {
                    host,
                    port,
                    password_key,
                    ..
                } => (
                    DbType::Redis,
                    host.clone(),
                    port.to_string(),
                    String::new(),
                    String::new(),
                    password_key.clone().unwrap_or_default(),
                    String::new(),
                    String::new(),
                ),
                DriverParams::MongoDB { uri, .. } => (
                    DbType::MongoDB,
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    uri.clone(),
                ),
                DriverParams::Mssql {
                    host,
                    port,
                    database,
                    user,
                    password_key,
                } => (
                    DbType::Mssql,
                    host.clone(),
                    port.to_string(),
                    database.clone(),
                    user.clone(),
                    password_key.clone(),
                    String::new(),
                    String::new(),
                ),
            };

        Self {
            edit_id: Some(config.id),
            name: config.name.clone(),
            db_type,
            host,
            port,
            database,
            username,
            password,
            sqlite_path,
            mongodb_uri,
            error: None,
        }
    }

    fn default_port(db_type: &DbType) -> &'static str {
        match db_type {
            DbType::Sqlite | DbType::MongoDB => "",
            DbType::Postgres => "5432",
            DbType::Mysql => "3306",
            DbType::Redis => "6379",
            DbType::Mssql => "1433",
        }
    }

    fn build_config(&self) -> Result<ConnectionConfig, String> {
        let name = if self.name.is_empty() {
            format!("{} @ {}", self.db_type.label(), self.host)
        } else {
            self.name.clone()
        };

        let params = match &self.db_type {
            DbType::Sqlite => {
                if self.sqlite_path.is_empty() {
                    return Err("SQLite path is required".into());
                }
                DriverParams::Sqlite {
                    path: std::path::PathBuf::from(&self.sqlite_path),
                }
            }
            DbType::Postgres => {
                let port: u16 = self.port.parse().map_err(|_| "Invalid port number")?;
                DriverParams::Postgres {
                    host: self.host.clone(),
                    port,
                    database: self.database.clone(),
                    user: self.username.clone(),
                    password_key: self.password.clone(),
                }
            }
            DbType::Mysql => {
                let port: u16 = self.port.parse().map_err(|_| "Invalid port number")?;
                DriverParams::Mysql {
                    host: self.host.clone(),
                    port,
                    database: self.database.clone(),
                    user: self.username.clone(),
                    password_key: self.password.clone(),
                }
            }
            DbType::Redis => {
                let port: u16 = self.port.parse().map_err(|_| "Invalid port number")?;
                DriverParams::Redis {
                    host: self.host.clone(),
                    port,
                    db_index: 0,
                    password_key: if self.password.is_empty() {
                        None
                    } else {
                        Some(self.password.clone())
                    },
                }
            }
            DbType::MongoDB => {
                if self.mongodb_uri.is_empty() {
                    return Err("MongoDB URI is required".into());
                }
                DriverParams::MongoDB {
                    uri: self.mongodb_uri.clone(),
                    password_key: None,
                }
            }
            DbType::Mssql => {
                let port: u16 = self.port.parse().map_err(|_| "Invalid port number")?;
                DriverParams::Mssql {
                    host: self.host.clone(),
                    port,
                    database: self.database.clone(),
                    user: self.username.clone(),
                    password_key: self.password.clone(),
                }
            }
        };

        let mut config = ConnectionConfig::new(&name, params);

        // Preserve the original id when editing so saved entries are updated in-place.
        if let Some(id) = self.edit_id {
            config.id = id;
        }

        Ok(config)
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
                        egui::ComboBox::from_id_salt("db_type")
                            .selected_text(self.db_type.label())
                            .show_ui(ui, |ui| {
                                for db_type in DbType::all() {
                                    let selected = &self.db_type == db_type;
                                    if ui.selectable_label(selected, db_type.label()).clicked()
                                        && !selected
                                    {
                                        self.db_type = db_type.clone();
                                        self.port = Self::default_port(&self.db_type).to_string();
                                    }
                                }
                            });
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
                                if ui.small_button("Browse…").clicked() {
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
                    if ui.button(confirm_label).clicked() {
                        match self.build_config() {
                            Ok(config) => {
                                result = DialogResult::Confirmed(config);
                                self.error = None;
                            }
                            Err(e) => self.error = Some(e),
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        result = DialogResult::Cancelled;
                    }
                });
            });

        result
    }
}
