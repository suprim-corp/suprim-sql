/// Tab manager — orchestrates tab lifecycle, routing to
/// SqlEditorTab / TableViewerTab / TableEditorTab implementations.
/// Tab bar rendering is delegated to `tab_bar.rs`.
use eframe::egui;
use suprim_core::db::commands::DbCommand;
use suprim_core::db::types::QueryResult;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::server_dashboard::ServerDashboardTab;
use super::shared::result_grid::build_display_cache;
use super::sql_editor::SqlEditorTab;
use super::tab_bar::render_tab_bar;
use super::table_editor_tab::TableEditorTab;
use super::table_viewer_tab::TableViewerTab;

// ── Tab kinds ────────────────────────────────────────────────────────────────

pub(super) enum TabKind {
    SqlEditor(SqlEditorTab),
    TableViewer(Box<TableViewerTab>),
    TableEditor(TableEditorTab),
    ServerDashboard(ServerDashboardTab),
}

// ── Tab entry ────────────────────────────────────────────────────────────────

pub(super) struct TabEntry {
    pub(super) tab_id: Uuid,
    pub(super) kind: TabKind,
    pub(super) conn_name: String,
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
    pub(super) tabs: Vec<TabEntry>,
    pub(super) active_tab: Option<Uuid>,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: None,
        }
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

    pub fn on_row_mutated(&mut self, tab_id: Uuid, _rows_affected: u64) {
        for entry in &mut self.tabs {
            if entry.tab_id == tab_id {
                if let TabKind::TableViewer(t) = &mut entry.kind {
                    t.needs_reload_after_mutation = true;
                }
                return;
            }
        }
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

    /// Get query info for any tab type (SQL editor or Table viewer).
    /// Returns (sql_text, conn_name, database) for history recording.
    pub fn get_tab_query_info(&self, tab_id: Uuid) -> Option<(String, String, Option<String>)> {
        for entry in &self.tabs {
            if entry.tab_id == tab_id {
                match &entry.kind {
                    TabKind::SqlEditor(t) => {
                        return Some((
                            t.sql_text().to_string(),
                            entry.conn_name.clone(),
                            t.database.clone(),
                        ));
                    }
                    TabKind::TableViewer(t) => {
                        let sql = build_table_viewer_sql(t);
                        return Some((sql, entry.conn_name.clone(), Some(t.database.clone())));
                    }
                    _ => return None,
                }
            }
        }
        None
    }

    /// Load SQL text into the active SQL editor tab.
    /// Returns false if no active SQL editor tab exists.
    pub fn load_sql_into_active_editor(&mut self, sql: &str) -> bool {
        if let Some(active_id) = self.active_tab {
            for entry in &mut self.tabs {
                if entry.tab_id == active_id {
                    if let TabKind::SqlEditor(t) = &mut entry.kind {
                        t.set_sql_text(sql);
                        return true;
                    }
                }
            }
        }
        false
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

/// Build a representative SQL string from TableViewerTab state.
fn build_table_viewer_sql(t: &TableViewerTab) -> String {
    let mut sql = format!("SELECT * FROM \"{}\".\"{}\"", t.schema_name, t.table_name);
    if !t.where_clause_text().is_empty() {
        sql.push_str(&format!(" WHERE {}", t.where_clause_text()));
    }
    if !t.order_clause_text().is_empty() {
        sql.push_str(&format!(" ORDER BY {}", t.order_clause_text()));
    }
    sql
}
