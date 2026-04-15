/// Connection dialog config building — maps dialog form fields to `ConnectionConfig`.
/// Also contains the `DbType` enum and `from_config` reverse-mapping.
use suprim_core::db::connection::{ConnectionConfig, DriverParams, SshConfig};
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
    // SSH tunnel fields
    pub ssh_enabled: bool,
    pub ssh_host: &'a str,
    pub ssh_port: &'a str,
    pub ssh_user: &'a str,
    pub ssh_key_path: &'a str,
    pub ssh_password: &'a str,
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

    // SSH tunnel
    if fields.ssh_enabled {
        let ssh_port: u16 = fields
            .ssh_port
            .parse()
            .map_err(|_| "Invalid SSH port number")?;
        if fields.ssh_host.is_empty() {
            return Err("SSH host is required".into());
        }
        if fields.ssh_user.is_empty() {
            return Err("SSH user is required".into());
        }
        config.ssh = Some(SshConfig {
            host: fields.ssh_host.to_string(),
            port: ssh_port,
            user: fields.ssh_user.to_string(),
            key_path: if fields.ssh_key_path.is_empty() {
                None
            } else {
                Some(std::path::PathBuf::from(fields.ssh_key_path))
            },
            password_key: if fields.ssh_password.is_empty() {
                None
            } else {
                Some(fields.ssh_password.to_string())
            },
        });
    }

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
    // SSH tunnel fields
    pub ssh_enabled: bool,
    pub ssh_host: String,
    pub ssh_port: String,
    pub ssh_user: String,
    pub ssh_key_path: String,
    pub ssh_password: String,
}

pub fn extract_fields(config: &ConnectionConfig) -> ExtractedFields {
    // Extract SSH fields once (shared across all driver types)
    let (ssh_enabled, ssh_host, ssh_port, ssh_user, ssh_key_path, ssh_password) =
        if let Some(ssh) = &config.ssh {
            (
                true,
                ssh.host.clone(),
                ssh.port.to_string(),
                ssh.user.clone(),
                ssh.key_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                ssh.password_key.clone().unwrap_or_default(),
            )
        } else {
            (
                false,
                String::new(),
                "22".to_string(),
                String::new(),
                String::new(),
                String::new(),
            )
        };

    let mut f = match &config.params {
        DriverParams::Sqlite { path } => ExtractedFields {
            db_type: DbType::Sqlite,
            host: String::new(),
            port: String::new(),
            database: String::new(),
            username: String::new(),
            password: String::new(),
            sqlite_path: path.to_string_lossy().to_string(),
            mongodb_uri: String::new(),
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: String::new(),
            ssh_user: String::new(),
            ssh_key_path: String::new(),
            ssh_password: String::new(),
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
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: String::new(),
            ssh_user: String::new(),
            ssh_key_path: String::new(),
            ssh_password: String::new(),
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
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: String::new(),
            ssh_user: String::new(),
            ssh_key_path: String::new(),
            ssh_password: String::new(),
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
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: String::new(),
            ssh_user: String::new(),
            ssh_key_path: String::new(),
            ssh_password: String::new(),
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
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: String::new(),
            ssh_user: String::new(),
            ssh_key_path: String::new(),
            ssh_password: String::new(),
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
            ssh_enabled: false,
            ssh_host: String::new(),
            ssh_port: String::new(),
            ssh_user: String::new(),
            ssh_key_path: String::new(),
            ssh_password: String::new(),
        },
    };

    // Apply SSH fields from config (shared across all driver types)
    f.ssh_enabled = ssh_enabled;
    f.ssh_host = ssh_host;
    f.ssh_port = ssh_port;
    f.ssh_user = ssh_user;
    f.ssh_key_path = ssh_key_path;
    f.ssh_password = ssh_password;
    f
}
