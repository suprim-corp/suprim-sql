//! Query history — persists executed queries to `~/.config/suprim-sql/query_history.json`.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single query history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub sql: String,
    pub conn_name: String,
    pub database: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub execution_time_ms: u64,
    pub row_count: usize,
    pub rows_affected: u64,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Persistent query history store — backed by a JSON file.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct QueryHistoryStore {
    pub entries: Vec<QueryHistoryEntry>,
}

/// Maximum number of entries to keep in history.
const MAX_ENTRIES: usize = 500;

impl QueryHistoryStore {
    fn history_path() -> PathBuf {
        let base = dirs_next::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("suprim-sql").join("query_history.json")
    }

    pub fn load() -> Self {
        let path = Self::history_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::history_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, text);
        }
    }

    /// Add a new entry (prepends, trims to MAX_ENTRIES, auto-saves).
    pub fn add(&mut self, entry: QueryHistoryEntry) {
        self.entries.insert(0, entry);
        if self.entries.len() > MAX_ENTRIES {
            self.entries.truncate(MAX_ENTRIES);
        }
        self.save();
    }

    /// Search entries by SQL text (case-insensitive substring match).
    pub fn search(&self, query: &str) -> Vec<&QueryHistoryEntry> {
        if query.is_empty() {
            return self.entries.iter().collect();
        }
        let q = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.sql.to_lowercase().contains(&q))
            .collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.save();
    }
}
