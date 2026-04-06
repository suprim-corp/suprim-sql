use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Which database engine this connection targets
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriverType {
    Sqlite,
    Postgres,
    Mysql,
    Redis,
    MongoDB,
    Mssql,
}

impl std::fmt::Display for DriverType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriverType::Sqlite => write!(f, "SQLite"),
            DriverType::Postgres => write!(f, "PostgreSQL"),
            DriverType::Mysql => write!(f, "MySQL"),
            DriverType::Redis => write!(f, "Redis"),
            DriverType::MongoDB => write!(f, "MongoDB"),
            DriverType::Mssql => write!(f, "MSSQL"),
        }
    }
}

/// Driver-specific connection parameters.
/// `password_key` is a key for OS keychain lookup — never store plaintext passwords.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DriverParams {
    Sqlite {
        path: std::path::PathBuf,
    },
    Postgres {
        host: String,
        port: u16,
        database: String,
        user: String,
        /// Key into OS keychain — use `keyring::Entry` to retrieve actual password
        password_key: String,
    },
    Mysql {
        host: String,
        port: u16,
        database: String,
        user: String,
        password_key: String,
    },
    Redis {
        host: String,
        port: u16,
        db_index: u8,
        password_key: Option<String>,
    },
    MongoDB {
        /// Connection URI (without password if password_key is set)
        uri: String,
        password_key: Option<String>,
    },
    Mssql {
        host: String,
        port: u16,
        database: String,
        user: String,
        password_key: String,
    },
}

impl DriverParams {
    pub fn driver_type(&self) -> DriverType {
        match self {
            DriverParams::Sqlite { .. } => DriverType::Sqlite,
            DriverParams::Postgres { .. } => DriverType::Postgres,
            DriverParams::Mysql { .. } => DriverType::Mysql,
            DriverParams::Redis { .. } => DriverType::Redis,
            DriverParams::MongoDB { .. } => DriverType::MongoDB,
            DriverParams::Mssql { .. } => DriverType::Mssql,
        }
    }
}

/// SSH tunnel configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    /// Path to private key file
    pub key_path: Option<std::path::PathBuf>,
    pub password_key: Option<String>,
}

/// TLS configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TlsConfig {
    pub enabled: bool,
    pub verify_cert: bool,
    pub ca_cert_path: Option<std::path::PathBuf>,
    pub client_cert_path: Option<std::path::PathBuf>,
    pub client_key_path: Option<std::path::PathBuf>,
}

/// A saved connection configuration.
/// Serialized to TOML, never contains plaintext passwords.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub id: Uuid,
    /// User-facing display name
    pub name: String,
    pub params: DriverParams,
    pub ssh: Option<SshConfig>,
    #[serde(default)]
    pub tls: TlsConfig,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
}

impl ConnectionConfig {
    pub fn new(name: impl Into<String>, params: DriverParams) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            params,
            ssh: None,
            tls: TlsConfig::default(),
            created_at: Utc::now(),
            last_used: None,
        }
    }

    pub fn driver_type(&self) -> DriverType {
        self.params.driver_type()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_type_display() {
        assert_eq!(DriverType::Postgres.to_string(), "PostgreSQL");
        assert_eq!(DriverType::Sqlite.to_string(), "SQLite");
        assert_eq!(DriverType::Mssql.to_string(), "MSSQL");
        assert_eq!(DriverType::Mysql.to_string(), "MySQL");
        assert_eq!(DriverType::Redis.to_string(), "Redis");
        assert_eq!(DriverType::MongoDB.to_string(), "MongoDB");
    }

    #[test]
    fn driver_params_returns_correct_driver_type() {
        let p = DriverParams::Sqlite {
            path: "/tmp/test.db".into(),
        };
        assert_eq!(p.driver_type(), DriverType::Sqlite);

        let p = DriverParams::Postgres {
            host: "localhost".into(),
            port: 5432,
            database: "mydb".into(),
            user: "admin".into(),
            password_key: "key-123".into(),
        };
        assert_eq!(p.driver_type(), DriverType::Postgres);
    }

    #[test]
    fn connection_config_new_assigns_uuid_and_timestamps() {
        let cfg = ConnectionConfig::new(
            "my db",
            DriverParams::Sqlite {
                path: "/tmp/test.db".into(),
            },
        );
        assert_eq!(cfg.name, "my db");
        assert!(cfg.last_used.is_none());
        assert_eq!(cfg.driver_type(), DriverType::Sqlite);
    }

    #[test]
    fn connection_config_serde_roundtrip() {
        let cfg = ConnectionConfig::new(
            "prod",
            DriverParams::Postgres {
                host: "db.example.com".into(),
                port: 5432,
                database: "app".into(),
                user: "app_user".into(),
                password_key: "conn-abc-123".into(),
            },
        );
        let toml_str = toml::to_string(&cfg).unwrap();
        let deserialized: ConnectionConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(cfg.id, deserialized.id);
        assert_eq!(cfg.name, deserialized.name);
        assert_eq!(cfg.driver_type(), deserialized.driver_type());
    }
}
