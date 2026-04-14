//! Workspace persistence — save/restore open tabs and UI state.
//!
//! Stored at `~/.config/suprim-sql/workspace.json`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Snapshot of a single tab's restorable state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum TabSnapshot {
    SqlEditor {
        tab_id: Uuid,
        conn_id: Option<Uuid>,
        conn_name: String,
        database: Option<String>,
        sql_text: String,
    },
    TableViewer {
        tab_id: Uuid,
        conn_id: Uuid,
        conn_name: String,
        database: String,
        schema_name: String,
        table_name: String,
        where_clause: String,
        order_clause: String,
        page: usize,
        page_size: usize,
    },
    ServerDashboard {
        tab_id: Uuid,
        conn_id: Uuid,
        conn_name: String,
        refresh_interval: f32,
        auto_refresh: bool,
    },
}

impl TabSnapshot {
    pub fn tab_id(&self) -> Uuid {
        match self {
            Self::SqlEditor { tab_id, .. }
            | Self::TableViewer { tab_id, .. }
            | Self::ServerDashboard { tab_id, .. } => *tab_id,
        }
    }

    pub fn conn_id(&self) -> Option<Uuid> {
        match self {
            Self::SqlEditor { conn_id, .. } => *conn_id,
            Self::TableViewer { conn_id, .. } => Some(*conn_id),
            Self::ServerDashboard { conn_id, .. } => Some(*conn_id),
        }
    }
}

/// Full workspace state persisted between sessions.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct WorkspaceState {
    /// Ordered list of open tabs.
    pub tabs: Vec<TabSnapshot>,
    /// Which tab was active (by tab_id).
    pub active_tab: Option<Uuid>,
    /// Connection IDs that were connected (for auto-reconnect).
    pub connected_ids: Vec<Uuid>,
    /// Whether the history panel was open.
    pub show_history: bool,
}

impl WorkspaceState {
    fn file_path() -> PathBuf {
        let base = dirs_next::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("suprim-sql").join("workspace.json")
    }

    pub fn load() -> Self {
        let path = Self::file_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::file_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, text);
        }
    }
}
