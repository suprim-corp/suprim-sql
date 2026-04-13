/// Tab manager — orchestrates tab lifecycle, routing to
/// SqlEditorTab / TableViewerTab / TableEditorTab implementations.
/// Tab bar rendering is delegated to `tab_bar.rs`.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::QueryResult;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::server_dashboard::ServerDashboardTab;
use super::shared::result_grid::build_display_cache;
use super::sql_editor::SqlEditorTab;
use super::tab_bar::render_tab_bar;
use super::table_editor_tab::TableEditorTab;
use super::table_viewer_tab::TableViewerTab;

// ── Tab kinds ────────────────────────────────────────────────────────────────

enum TabKind {
    SqlEditor(SqlEditorTab),
    TableViewer(TableViewerTab),
    TableEditor(TableEditorTab),
    ServerDashboard(ServerDashboardTab),
}

// ── Tab entry ────────────────────────────────────────────────────────────────

struct TabEntry {
    tab_id: Uuid,
    kind: TabKind,
    conn_name: String,
}

impl TabEntry {
    fn tab_label(&self) -> String {
        let icon = match &self.kind {
            TabKind::SqlEditor(_) => egui_phosphor::regular::TERMINAL_WINDOW,
            TabKind::TableViewer(_) => egui_phosphor::regular::TABLE,
            TabKind::TableEditor(_) => egui_phosphor::regular::PENCIL_SIMPLE,
            TabKind::ServerDashboard(_) => egui_phosphor::regular::GAUGE,
        };
        let name = match &self.kind {
            TabKind::SqlEditor(_) => "Query".to_string(),
            TabKind::TableViewer(t) => truncate_str(&t.table_name, 18),
            TabKind::TableEditor(t) => {
                if t.is_new_table {
                    "New Table".to_string()
                } else {
                    format!("Edit: {}", truncate_str(&t.table_name, 14))
                }
            }
            TabKind::ServerDashboard(_) => "Dashboard".to_string(),
        };
        let conn = truncate_str(&self.conn_name, 20);
        format!("{icon} {name} [{conn}]")
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let half = max / 2;
        let start: String = s.chars().take(half).collect();
        let end: String = s
            .chars()
            .rev()
            .take(half)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("{start}...{end}")
    }
}

// ── TabManager ───────────────────────────────────────────────────────────────

pub struct TabManager {
    tabs: Vec<TabEntry>,
    active_tab: Option<Uuid>,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: None,
        }
    }

    pub fn open_sql_tab(
        &mut self,
        conn_id: Option<Uuid>,
        conn_name: String,
        database: Option<String>,
        databases: Vec<String>,
    ) {
        let tab_id = Uuid::new_v4();
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::SqlEditor(SqlEditorTab::new(conn_id, database, databases)),
            conn_name,
        });
        self.active_tab = Some(tab_id);
    }

    pub fn open_table_viewer(
        &mut self,
        conn_id: Uuid,
        conn_name: String,
        database: String,
        schema_name: String,
        table_name: String,
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
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::TableViewer(TableViewerTab::new(
                conn_id,
                database,
                schema_name,
                table_name,
            )),
            conn_name,
        });
        self.active_tab = Some(tab_id);
    }

    pub fn open_table_editor(
        &mut self,
        conn_id: Uuid,
        conn_name: String,
        database: String,
        schema_name: String,
        table: &suprim_sql::db::types::TableNode,
        schema_functions: Vec<String>,
    ) {
        let tab_id = Uuid::new_v4();
        let mut editor = TableEditorTab::new(conn_id, database, schema_name, table);
        editor.schema_functions = schema_functions;
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::TableEditor(editor),
            conn_name,
        });
        self.active_tab = Some(tab_id);
    }

    pub fn open_new_table_editor(
        &mut self,
        conn_id: Uuid,
        conn_name: String,
        database: String,
        schema_name: String,
        schema_functions: Vec<String>,
    ) {
        let tab_id = Uuid::new_v4();
        let mut editor = TableEditorTab::new_empty(conn_id, database, schema_name);
        editor.schema_functions = schema_functions;
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

    pub fn on_query_result(&mut self, tab_id: Uuid, result: QueryResult) {
        for entry in &mut self.tabs {
            if entry.tab_id == tab_id {
                let cache = build_display_cache(&result);
                match &mut entry.kind {
                    TabKind::SqlEditor(t) => t.set_result(result, cache),
                    TabKind::TableViewer(t) => t.set_result(result, cache),
                    TabKind::TableEditor(_) | TabKind::ServerDashboard(_) => {}
                }
                return;
            }
        }
    }

    pub fn on_row_mutated(&mut self, _tab_id: Uuid, _rows_affected: u64) {
        // Could refresh the table viewer here.
    }

    /// Notify a tab that its query failed.
    pub fn on_tab_error(&mut self, tab_id: Uuid) {
        for entry in &mut self.tabs {
            if entry.tab_id == tab_id {
                if let TabKind::SqlEditor(t) = &mut entry.kind {
                    t.on_error();
                }
                return;
            }
        }
    }

    /// Returns true if any tab is currently waiting for a DB response.
    pub fn any_tab_loading(&self) -> bool {
        self.tabs.iter().any(|entry| match &entry.kind {
            TabKind::SqlEditor(t) => t.is_running,
            TabKind::TableViewer(t) => t.is_loading,
            TabKind::TableEditor(_) => false,
            TabKind::ServerDashboard(_) => false, // dashboard uses its own repaint timer
        })
    }

    /// Close all tabs associated with a specific connection.
    pub fn close_tabs_for_connection(&mut self, conn_id: Uuid) {
        self.tabs.retain(|entry| {
            let tab_conn = match &entry.kind {
                TabKind::SqlEditor(t) => t.conn_id,
                TabKind::TableViewer(t) => Some(t.conn_id),
                TabKind::TableEditor(t) => Some(t.conn_id),
                TabKind::ServerDashboard(t) => Some(t.conn_id),
            };
            tab_conn != Some(conn_id)
        });
        // If active tab was closed, switch to the last remaining tab.
        if let Some(active) = self.active_tab {
            if !self.tabs.iter().any(|t| t.tab_id == active) {
                self.active_tab = self.tabs.last().map(|t| t.tab_id);
            }
        }
    }

    /// Update a dashboard tab with fresh data from the DB worker.
    pub fn on_dashboard_loaded(
        &mut self,
        conn_id: Uuid,
        sessions: Vec<suprim_sql::db::schema::SessionInfo>,
        metrics: suprim_sql::db::schema::ServerMetrics,
    ) {
        for entry in &mut self.tabs {
            if let TabKind::ServerDashboard(d) = &mut entry.kind {
                if d.conn_id == conn_id {
                    d.on_data_loaded(sessions, metrics);
                    return;
                }
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, cmd_tx: &mpsc::Sender<DbCommand>) {
        if self.tabs.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    ui.heading("SuprimSQL");
                    ui.add_space(12.0);
                    ui.label("Open a connection from the sidebar to get started.");
                });
            });
            return;
        }

        // Collect tab data for the bar renderer
        let tab_data: Vec<(Uuid, String)> = self
            .tabs
            .iter()
            .map(|e| (e.tab_id, e.tab_label()))
            .collect();

        let bar_out = render_tab_bar(ui, &tab_data, self.active_tab);

        if let Some(id) = bar_out.activated {
            self.active_tab = Some(id);
        }

        ui.separator();

        // Close tab if requested
        if let Some(id) = bar_out.closed {
            self.tabs.retain(|t| t.tab_id != id);
            if self.active_tab == Some(id) {
                self.active_tab = self.tabs.last().map(|t| t.tab_id);
            }
        }

        // Show active tab content
        if let Some(active_id) = self.active_tab {
            for entry in &mut self.tabs {
                if entry.tab_id == active_id {
                    let tab_id = entry.tab_id;
                    match &mut entry.kind {
                        TabKind::SqlEditor(t) => t.show(ui, tab_id, cmd_tx),
                        TabKind::TableViewer(t) => t.show(ui, tab_id, cmd_tx),
                        TabKind::TableEditor(t) => t.show(ui, tab_id, cmd_tx),
                        TabKind::ServerDashboard(t) => t.show(ui, cmd_tx),
                    }
                    break;
                }
            }
        }
    }
}
