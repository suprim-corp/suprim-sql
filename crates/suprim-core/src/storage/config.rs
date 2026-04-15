use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::db::connection::{ConnectionConfig, DriverParams};
use crate::storage::credential;

/// Persisted app configuration — saved to `~/.config/suprim-sql/connections.toml`
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub connections: Vec<ConnectionConfig>,
}

impl AppConfig {
    fn config_path() -> PathBuf {
        let base = dirs_next::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("suprim-sql").join("connections.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let mut config: Self = toml::from_str(&text).unwrap_or_default();

        // Auto-migrate: encrypt any plain text passwords
        if config.migrate_plain_passwords() {
            config.save();
        }

        config
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = toml::to_string_pretty(self) {
            let _ = std::fs::write(&path, text);
        }
    }

    pub fn add_connection(&mut self, mut config: ConnectionConfig) {
        // Encrypt password before saving
        encrypt_connection_passwords(&mut config);

        if let Some(pos) = self.connections.iter().position(|c| c.id == config.id) {
            self.connections[pos] = config;
        } else {
            self.connections.push(config);
        }
        self.save();
    }

    pub fn remove_connection(&mut self, id: uuid::Uuid) {
        self.connections.retain(|c| c.id != id);
        self.save();
    }

    /// Scan all connections for plain-text passwords and encrypt them.
    /// Returns `true` if any were migrated.
    fn migrate_plain_passwords(&mut self) -> bool {
        let mut migrated = false;
        for conn in &mut self.connections {
            if encrypt_connection_passwords(conn) {
                migrated = true;
            }
        }
        if migrated {
            tracing::info!("Migrated plain-text passwords to encrypted storage");
        }
        migrated
    }
}

/// Encrypt all password fields in a connection config.
/// Returns `true` if any field was encrypted (was plain text before).
fn encrypt_connection_passwords(conn: &mut ConnectionConfig) -> bool {
    let mut changed = false;

    match &mut conn.params {
        DriverParams::Postgres { password_key, .. }
        | DriverParams::Mysql { password_key, .. }
        | DriverParams::Mssql { password_key, .. } => {
            if !password_key.is_empty() && !credential::is_encrypted(password_key) {
                *password_key = credential::encrypt(password_key);
                changed = true;
            }
        }
        DriverParams::Redis { password_key, .. } | DriverParams::MongoDB { password_key, .. } => {
            if let Some(pw) = password_key {
                if !pw.is_empty() && !credential::is_encrypted(pw) {
                    *pw = credential::encrypt(pw);
                    changed = true;
                }
            }
        }
        DriverParams::Sqlite { .. } => {}
    }

    // SSH password
    if let Some(ssh) = &mut conn.ssh {
        if let Some(pw) = &mut ssh.password_key {
            if !pw.is_empty() && !credential::is_encrypted(pw) {
                *pw = credential::encrypt(pw);
                changed = true;
            }
        }
    }

    changed
}
