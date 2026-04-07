/// Connection dialog config building — maps dialog form fields to `ConnectionConfig`.
/// Also contains the `DbType` enum and `from_config` reverse-mapping.
use suprim_sql::db::connection::{ConnectionConfig, DriverParams};
use uuid::Uuid;

/// Which database type is selected in the dialog.
#[derive(Debug, Clone, PartialEq)]
pub enum DbType {
    Sqlite,
    Postgres,
    Mysql,
    Redis,
    MongoDB,
    Mssql,
}

impl DbType {
    pub fn label(&self) -> &str {
        match self {
            DbType::Sqlite => "SQLite",
            DbType::Postgres => "PostgreSQL",
            DbType::Mysql => "MySQL / MariaDB",
            DbType::Redis => "Redis",
            DbType::MongoDB => "MongoDB",
            DbType::Mssql => "MSSQL / Azure",
        }
    }

    pub fn all() -> &'static [DbType] {
        &[
            DbType::Sqlite,
            DbType::Postgres,
            DbType::Mysql,
            DbType::Redis,
            DbType::MongoDB,
            DbType::Mssql,
        ]
    }

    pub fn default_port(&self) -> &'static str {
        match self {
            DbType::Sqlite | DbType::MongoDB => "",
            DbType::Postgres => "5432",
            DbType::Mysql => "3306",
            DbType::Redis => "6379",
            DbType::Mssql => "1433",
        }
    }
}

/// Raw form fields needed to build a config. Passed by reference from the dialog.
pub struct DialogFields<'a> {
    pub edit_id: Option<Uuid>,
    pub name: &'a str,
    pub db_type: &'a DbType,
    pub host: &'a str,
    pub port: &'a str,
    pub database: &'a str,
    pub username: &'a str,
    pub password: &'a str,
    pub sqlite_path: &'a str,
    pub mongodb_uri: &'a str,
}

/// Build a `ConnectionConfig` from dialog form fields, validating inputs.
pub fn build_config(fields: &DialogFields<'_>) -> Result<ConnectionConfig, String> {
    let name = if fields.name.is_empty() {
        format!("{} @ {}", fields.db_type.label(), fields.host)
    } else {
        fields.name.to_string()
    };

    let params = match fields.db_type {
        DbType::Sqlite => {
            if fields.sqlite_path.is_empty() {
                return Err("SQLite path is required".into());
            }
            DriverParams::Sqlite {
                path: std::path::PathBuf::from(fields.sqlite_path),
            }
        }
        DbType::Postgres => {
            let port: u16 = fields.port.parse().map_err(|_| "Invalid port number")?;
            DriverParams::Postgres {
                host: fields.host.to_string(),
                port,
                database: fields.database.to_string(),
                user: fields.username.to_string(),
                password_key: fields.password.to_string(),
            }
        }
        DbType::Mysql => {
            let port: u16 = fields.port.parse().map_err(|_| "Invalid port number")?;
            DriverParams::Mysql {
                host: fields.host.to_string(),
                port,
                database: fields.database.to_string(),
                user: fields.username.to_string(),
                password_key: fields.password.to_string(),
            }
        }
        DbType::Redis => {
            let port: u16 = fields.port.parse().map_err(|_| "Invalid port number")?;
            DriverParams::Redis {
                host: fields.host.to_string(),
                port,
                db_index: 0,
                password_key: if fields.password.is_empty() {
                    None
                } else {
                    Some(fields.password.to_string())
                },
            }
        }
        DbType::MongoDB => {
            if fields.mongodb_uri.is_empty() {
                return Err("MongoDB URI is required".into());
            }
            DriverParams::MongoDB {
                uri: fields.mongodb_uri.to_string(),
                password_key: None,
            }
        }
        DbType::Mssql => {
            let port: u16 = fields.port.parse().map_err(|_| "Invalid port number")?;
            DriverParams::Mssql {
                host: fields.host.to_string(),
                port,
                database: fields.database.to_string(),
                user: fields.username.to_string(),
                password_key: fields.password.to_string(),
            }
        }
    };

    let mut config = ConnectionConfig::new(&name, params);

    // Preserve the original id when editing so saved entries are updated in-place.
    if let Some(id) = fields.edit_id {
        config.id = id;
    }

    Ok(config)
}

/// Extract dialog field values from an existing `ConnectionConfig` for editing.
pub struct ExtractedFields {
    pub db_type: DbType,
    pub host: String,
    pub port: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub sqlite_path: String,
    pub mongodb_uri: String,
}

pub fn extract_fields(config: &ConnectionConfig) -> ExtractedFields {
    match &config.params {
        DriverParams::Sqlite { path } => ExtractedFields {
            db_type: DbType::Sqlite,
            host: String::new(),
            port: String::new(),
            database: String::new(),
            username: String::new(),
            password: String::new(),
            sqlite_path: path.to_string_lossy().to_string(),
            mongodb_uri: String::new(),
        },
        DriverParams::Postgres {
            host,
            port,
            database,
            user,
            password_key,
        } => ExtractedFields {
            db_type: DbType::Postgres,
            host: host.clone(),
            port: port.to_string(),
            database: database.clone(),
            username: user.clone(),
            password: password_key.clone(),
            sqlite_path: String::new(),
            mongodb_uri: String::new(),
        },
        DriverParams::Mysql {
            host,
            port,
            database,
            user,
            password_key,
        } => ExtractedFields {
            db_type: DbType::Mysql,
            host: host.clone(),
            port: port.to_string(),
            database: database.clone(),
            username: user.clone(),
            password: password_key.clone(),
            sqlite_path: String::new(),
            mongodb_uri: String::new(),
        },
        DriverParams::Redis {
            host,
            port,
            password_key,
            ..
        } => ExtractedFields {
            db_type: DbType::Redis,
            host: host.clone(),
            port: port.to_string(),
            database: String::new(),
            username: String::new(),
            password: password_key.clone().unwrap_or_default(),
            sqlite_path: String::new(),
            mongodb_uri: String::new(),
        },
        DriverParams::MongoDB { uri, .. } => ExtractedFields {
            db_type: DbType::MongoDB,
            host: String::new(),
            port: String::new(),
            database: String::new(),
            username: String::new(),
            password: String::new(),
            sqlite_path: String::new(),
            mongodb_uri: uri.clone(),
        },
        DriverParams::Mssql {
            host,
            port,
            database,
            user,
            password_key,
        } => ExtractedFields {
            db_type: DbType::Mssql,
            host: host.clone(),
            port: port.to_string(),
            database: database.clone(),
            username: user.clone(),
            password: password_key.clone(),
            sqlite_path: String::new(),
            mongodb_uri: String::new(),
        },
    }
}
