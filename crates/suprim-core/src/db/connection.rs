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

/// SSL/TLS mode for database connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SslMode {
    /// No SSL, even if server supports it.
    Disable,
    /// Use SSL if server supports it, fallback to plain if not (default).
    #[default]
    Prefer,
    /// Require SSL, fail if server doesn't support it. No cert verification.
    Require,
    /// Require SSL + verify server certificate against CA.
    VerifyCa,
}

impl SslMode {
    pub fn label(&self) -> &'static str {
        match self {
            SslMode::Disable => "Disable",
            SslMode::Prefer => "Prefer",
            SslMode::Require => "Require",
            SslMode::VerifyCa => "Verify CA",
        }
    }

    pub fn all() -> &'static [SslMode] {
        &[
            SslMode::Disable,
            SslMode::Prefer,
            SslMode::Require,
            SslMode::VerifyCa,
        ]
    }
}

/// TLS configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct TlsConfig {
    #[serde(default)]
    pub ssl_mode: SslMode,
    pub ca_cert_path: Option<std::path::PathBuf>,
    pub client_cert_path: Option<std::path::PathBuf>,
    pub client_key_path: Option<std::path::PathBuf>,

    // Legacy fields — read from old config files, not written to new ones.
    // Migrated to ssl_mode on next save.
    #[serde(default, skip_serializing)]
    enabled: bool,
    #[serde(default, skip_serializing)]
    verify_cert: bool,
}

impl TlsConfig {
    /// Migrate legacy `enabled`/`verify_cert` fields to `ssl_mode`.
    /// Returns true if migration was performed.
    pub fn migrate_legacy(&mut self) -> bool {
        if self.enabled && self.ssl_mode == SslMode::Prefer {
            // Old config had enabled=true but ssl_mode is still default (Prefer)
            // → must have been written by old version
            self.ssl_mode = if self.verify_cert {
                SslMode::VerifyCa
            } else {
                SslMode::Require
            };
            self.enabled = false;
            self.verify_cert = false;
            return true;
        }
        false
    }
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
    /// For Postgres: only show these databases in the sidebar.
    /// None = show all databases (default). Empty vec = show all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_databases: Option<Vec<String>>,
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
            visible_databases: None,
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
