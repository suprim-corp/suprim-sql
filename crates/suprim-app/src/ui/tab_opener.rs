/// Tab opening methods — creates and pushes new tabs into the TabManager.
/// Extracted from `tab_manager.rs` to keep that file focused on lifecycle + rendering.
use suprim_core::db::dialect::SqlDialect;
use uuid::Uuid;

use super::server_dashboard::ServerDashboardTab;
use super::sql_editor::SqlEditorTab;
use super::tab_manager::{TabEntry, TabKind, TabManager};
use super::table_editor_tab::TableEditorTab;
use super::table_viewer_tab::TableViewerTab;

impl TabManager {
    pub fn open_sql_tab(
        &mut self,
        conn_id: Option<Uuid>,
        conn_name: String,
        database: Option<String>,
        databases: Vec<String>,
    ) {
        self.open_sql_tab_with_dialect(
            conn_id,
            conn_name,
            database,
            databases,
            SqlDialect::default(),
        )
    }

    pub fn open_sql_tab_with_dialect(
        &mut self,
        conn_id: Option<Uuid>,
        conn_name: String,
        database: Option<String>,
        databases: Vec<String>,
        dialect: SqlDialect,
    ) {
        let tab_id = Uuid::new_v4();
        let mut tab = SqlEditorTab::new(conn_id, database, databases);
        tab.dialect = dialect;
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::SqlEditor(tab),
            conn_name,
        });
        self.active_tab = Some(tab_id);
    }

    pub fn open_table_viewer_with_dialect(
        &mut self,
        conn_id: Uuid,
        conn_name: String,
        database: String,
        schema_name: String,
        table_name: String,
        dialect: SqlDialect,
    ) {
        // If a viewer for this exact table is already open, just activate it.
        for entry in &self.tabs {
            if let TabKind::TableViewer(t) = &entry.kind {
                if t.conn_id == conn_id
                    && t.database == database
                    && t.schema_name == schema_name
                    && t.table_name == table_name
                {
                    self.active_tab = Some(entry.tab_id);
                    return;
                }
            }
        }

        let tab_id = Uuid::new_v4();
        let mut tab = TableViewerTab::new(conn_id, database, schema_name, table_name);
        tab.dialect = dialect;
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::TableViewer(Box::new(tab)),
            conn_name,
        });
        self.active_tab = Some(tab_id);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn open_table_editor_with_dialect(
        &mut self,
        conn_id: Uuid,
        conn_name: String,
        database: String,
        schema_name: String,
        table: &suprim_core::db::types::TableNode,
        schema_functions: Vec<String>,
        dialect: SqlDialect,
    ) {
        let tab_id = Uuid::new_v4();
        let mut editor = TableEditorTab::new(conn_id, database, schema_name, table);
        editor.schema_functions = schema_functions;
        editor.dialect = dialect;
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::TableEditor(editor),
            conn_name,
        });
        self.active_tab = Some(tab_id);
    }

    pub fn open_new_table_editor_with_dialect(
        &mut self,
        conn_id: Uuid,
        conn_name: String,
        database: String,
        schema_name: String,
        schema_functions: Vec<String>,
        dialect: SqlDialect,
    ) {
        let tab_id = Uuid::new_v4();
        let mut editor = TableEditorTab::new_empty(conn_id, database, schema_name);
        editor.schema_functions = schema_functions;
        editor.dialect = dialect;
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::TableEditor(editor),
            conn_name,
        });
        self.active_tab = Some(tab_id);
    }

    /// Open a Server Dashboard tab for a connection.
    /// Reuses an existing dashboard tab if one is already open for this connection.
    pub fn open_server_dashboard(&mut self, conn_id: Uuid, conn_name: String) {
        // Reuse existing dashboard tab for this connection
        for entry in &self.tabs {
            if let TabKind::ServerDashboard(d) = &entry.kind {
                if d.conn_id == conn_id {
                    self.active_tab = Some(entry.tab_id);
                    return;
                }
            }
        }
        let tab_id = Uuid::new_v4();
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::ServerDashboard(ServerDashboardTab::new(conn_id)),
            conn_name,
        });
        self.active_tab = Some(tab_id);
    }

    /// Update a dashboard tab with fresh data from the DB worker.
    pub fn on_dashboard_loaded(
        &mut self,
        conn_id: Uuid,
        sessions: Vec<suprim_core::db::schema::SessionInfo>,
        metrics: suprim_core::db::schema::ServerMetrics,
        slow_queries: Vec<suprim_core::db::schema::SlowQueryInfo>,
    ) {
        for entry in &mut self.tabs {
            if let TabKind::ServerDashboard(d) = &mut entry.kind {
                if d.conn_id == conn_id {
                    d.on_data_loaded(sessions, metrics, slow_queries);
                    return;
                }
            }
        }
    }
}
