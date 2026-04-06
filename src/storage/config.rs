use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::db::connection::ConnectionConfig;

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
        toml::from_str(&text).unwrap_or_default()
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

    pub fn add_connection(&mut self, config: ConnectionConfig) {
        // Replace if same id exists, otherwise push.
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
}
