//! Tab workspace snapshot and restore — serialization of open tabs for persistence.

use suprim_sql::storage::TabSnapshot;
use uuid::Uuid;

use super::server_dashboard::ServerDashboardTab;
use super::sql_editor::SqlEditorTab;
use super::tab_manager::{TabEntry, TabKind, TabManager};
use super::table_viewer_tab::TableViewerTab;

impl TabManager {
    /// Create a snapshot of all open tabs for workspace persistence.
    pub fn snapshot(&self) -> (Vec<TabSnapshot>, Option<Uuid>) {
        let tabs: Vec<TabSnapshot> = self
            .tabs
            .iter()
            .filter_map(|entry| {
                let conn_name = entry.conn_name.clone();
                match &entry.kind {
                    TabKind::SqlEditor(t) => Some(TabSnapshot::SqlEditor {
                        tab_id: entry.tab_id,
                        conn_id: t.conn_id,
                        conn_name,
                        database: t.database.clone(),
                        sql_text: t.sql_text().to_string(),
                    }),
                    TabKind::TableViewer(t) => Some(TabSnapshot::TableViewer {
                        tab_id: entry.tab_id,
                        conn_id: t.conn_id,
                        conn_name,
                        database: t.database.clone(),
                        schema_name: t.schema_name.clone(),
                        table_name: t.table_name.clone(),
                        where_clause: t.where_clause.clone(),
                        order_clause: t.order_clause.clone(),
                        page: t.page,
                        page_size: t.page_size,
                    }),
                    TabKind::ServerDashboard(d) => Some(TabSnapshot::ServerDashboard {
                        tab_id: entry.tab_id,
                        conn_id: d.conn_id,
                        conn_name,
                        refresh_interval: d.refresh_interval,
                        auto_refresh: d.auto_refresh,
                    }),
                    // TableEditor tabs contain unsaved design — skip to avoid data loss ambiguity
                    TabKind::TableEditor(_) => None,
                }
            })
            .collect();
        (tabs, self.active_tab)
    }

    /// Restore tabs from a workspace snapshot.
    /// Tabs whose connection is missing from config are silently skipped.
    pub fn restore_from(&mut self, snapshots: Vec<TabSnapshot>, active_tab: Option<Uuid>) {
        for snap in snapshots {
            let entry = match snap {
                TabSnapshot::SqlEditor {
                    tab_id,
                    conn_id,
                    conn_name,
                    database,
                    sql_text,
                } => {
                    let mut tab = SqlEditorTab::new(conn_id, database.clone(), Vec::new());
                    tab.set_sql_text(&sql_text);
                    TabEntry {
                        tab_id,
                        kind: TabKind::SqlEditor(tab),
                        conn_name,
                    }
                }
                TabSnapshot::TableViewer {
                    tab_id,
                    conn_id,
                    conn_name,
                    database,
                    schema_name,
                    table_name,
                    where_clause,
                    order_clause,
                    page,
                    page_size,
                } => {
                    let mut tab = TableViewerTab::new(conn_id, database, schema_name, table_name);
                    tab.where_clause = where_clause;
                    tab.order_clause = order_clause;
                    tab.page = page;
                    tab.page_size = page_size;
                    // Will auto-load data once connection is established
                    TabEntry {
                        tab_id,
                        kind: TabKind::TableViewer(Box::new(tab)),
                        conn_name,
                    }
                }
                TabSnapshot::ServerDashboard {
                    tab_id,
                    conn_id,
                    conn_name,
                    refresh_interval,
                    auto_refresh,
                } => {
                    let mut tab = ServerDashboardTab::new(conn_id);
                    tab.refresh_interval = refresh_interval;
                    tab.auto_refresh = auto_refresh;
                    TabEntry {
                        tab_id,
                        kind: TabKind::ServerDashboard(tab),
                        conn_name,
                    }
                }
            };
            self.tabs.push(entry);
        }
        // Set active tab (if it exists in restored tabs)
        if let Some(id) = active_tab {
            if self.tabs.iter().any(|t| t.tab_id == id) {
                self.active_tab = Some(id);
            }
        }
        if self.active_tab.is_none() {
            self.active_tab = self.tabs.first().map(|t| t.tab_id);
        }
    }
}
