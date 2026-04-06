use eframe::egui;
use suprim_sql::db::driver::{DbCommand, DbEvent};
use suprim_sql::storage::AppConfig;
use tokio::sync::mpsc;

use crate::ui::{ConnectionDialog, Sidebar, StatusBar, TabManager};

/// Main application state — owned by the eframe runtime on the UI thread.
pub struct App {
    /// Sender to the background DbWorker task.
    cmd_tx: mpsc::Sender<DbCommand>,
    /// Receiver for events coming back from the DbWorker.
    event_rx: mpsc::Receiver<DbEvent>,

    sidebar: Sidebar,
    tab_manager: TabManager,
    statusbar: StatusBar,

    /// Optional modal dialog currently shown.
    connection_dialog: Option<ConnectionDialog>,

    /// Status message shown in the status bar.
    status: String,

    /// Persisted app configuration (connections list).
    config: AppConfig,
}

impl App {
    pub fn with_channels(
        _cc: &eframe::CreationContext<'_>,
        cmd_tx: mpsc::Sender<DbCommand>,
        event_rx: mpsc::Receiver<DbEvent>,
    ) -> Self {
        // Load saved connections from disk.
        let config = AppConfig::load();

        // Auto-reconnect all saved connections.
        for conn in &config.connections {
            let _ = cmd_tx.try_send(DbCommand::Connect {
                config: conn.clone(),
            });
        }

        Self {
            cmd_tx,
            event_rx,
            sidebar: Sidebar::new(),
            tab_manager: TabManager::new(),
            statusbar: StatusBar::new(),
            connection_dialog: None,
            status: if config.connections.is_empty() {
                "Ready".to_string()
            } else {
                format!(
                    "Reconnecting {} saved connection(s)…",
                    config.connections.len()
                )
            },
            config,
        }
    }

    /// Drain all pending events from the DbWorker and update state.
    fn process_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                DbEvent::Connected { conn_id, schema } => {
                    // Use the saved connection name; fall back to conn_id string.
                    let name = self
                        .config
                        .connections
                        .iter()
                        .find(|c| c.id == conn_id)
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| conn_id.to_string());
                    self.sidebar.on_connected(conn_id, name, schema);
                    self.status = "Connected".to_string();
                }
                DbEvent::Disconnected { conn_id } => {
                    self.sidebar.on_disconnected(conn_id);
                    self.config.remove_connection(conn_id);
                    self.status = "Disconnected".to_string();
                }
                DbEvent::QueryResult { tab_id, result } => {
                    let row_count = result.rows.len();
                    let millis = result.execution_time.as_millis();
                    self.tab_manager.on_query_result(tab_id, result);
                    self.status = format!("Query complete — {row_count} rows  ({millis} ms)");
                }
                DbEvent::SchemaLoaded { conn_id, schema } => {
                    self.sidebar.on_schema_loaded(conn_id, schema);
                }
                DbEvent::SchemaDetailLoaded {
                    conn_id,
                    schema_name,
                    schema_node,
                } => {
                    self.sidebar
                        .on_schema_detail_loaded(conn_id, &schema_name, schema_node);
                }
                DbEvent::RowMutated {
                    tab_id,
                    rows_affected,
                } => {
                    self.tab_manager.on_row_mutated(tab_id, rows_affected);
                    self.status = format!("{rows_affected} row(s) affected");
                }
                DbEvent::Error { message, .. } => {
                    self.status = format!("Error: {message}");
                }
            }
        }
    }
}

impl eframe::App for App {
    /// Process DB events (called before rendering each frame).
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_events();
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // ── Top menu bar ────────────────────────────────────────────────
        egui::Panel::top("menu_bar").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Connection", |ui| {
                    if ui.button("New Connection…").clicked() {
                        self.connection_dialog = Some(ConnectionDialog::new());
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Query", |ui| {
                    if ui.button("New SQL Tab").clicked() {
                        self.tab_manager.open_sql_tab(None);
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui.button("Reload Schema").clicked() {
                        for conn_id in self.sidebar.active_connection_ids() {
                            let _ = self.cmd_tx.try_send(DbCommand::LoadSchema { conn_id });
                        }
                        ui.close();
                    }
                });
            });
        });

        // ── Status bar (bottom) ─────────────────────────────────────────
        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            self.statusbar.show(ui, &self.status);
        });

        // ── Sidebar (left) ──────────────────────────────────────────────
        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(220.0)
            .min_size(160.0)
            .show_inside(ui, |ui| {
                let action = self.sidebar.show(ui);
                if let Some(act) = action {
                    use crate::ui::SidebarAction;
                    match act {
                        SidebarAction::NewConnection => {
                            self.connection_dialog = Some(ConnectionDialog::new());
                        }
                        SidebarAction::EditConnection { conn_id } => {
                            // Pre-populate dialog with current connection data.
                            if let Some(cfg) =
                                self.config.connections.iter().find(|c| c.id == conn_id)
                            {
                                self.connection_dialog = Some(ConnectionDialog::from_config(cfg));
                            }
                        }
                        SidebarAction::OpenSqlTab { conn_id } => {
                            self.tab_manager.open_sql_tab(Some(conn_id));
                        }
                        SidebarAction::OpenTableViewer {
                            conn_id,
                            table_name,
                        } => {
                            self.tab_manager.open_table_viewer(conn_id, table_name);
                        }
                        SidebarAction::Disconnect { conn_id } => {
                            let _ = self.cmd_tx.try_send(DbCommand::Disconnect { conn_id });
                        }
                        SidebarAction::LoadSchemaDetail {
                            conn_id,
                            schema_name,
                        } => {
                            let _ = self.cmd_tx.try_send(DbCommand::LoadSchemaDetail {
                                conn_id,
                                schema_name,
                            });
                        }
                        SidebarAction::UpdateVisibleDatabases { conn_id, visible } => {
                            // Persist the filter into the connection config.
                            if let Some(cfg) =
                                self.config.connections.iter_mut().find(|c| c.id == conn_id)
                            {
                                cfg.visible_databases = visible;
                                self.config.save();
                            }
                            // Reload schema so the worker can re-apply the filter.
                            let _ = self.cmd_tx.try_send(DbCommand::LoadSchema { conn_id });
                        }
                    }
                }
            });

        // ── Main content area ───────────────────────────────────────────
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.tab_manager.show(ui, &self.cmd_tx);
        });

        // ── Connection dialog (modal) ───────────────────────────────────
        let mut close_dialog = false;
        if let Some(dialog) = &mut self.connection_dialog {
            let result = dialog.show(&ctx);
            match result {
                crate::ui::DialogResult::Pending => {}
                crate::ui::DialogResult::Cancelled => close_dialog = true,
                crate::ui::DialogResult::Confirmed(config) => {
                    // If editing an existing connection, disconnect the old one first.
                    let is_edit = self.config.connections.iter().any(|c| c.id == config.id);
                    if is_edit {
                        let _ = self
                            .cmd_tx
                            .try_send(DbCommand::Disconnect { conn_id: config.id });
                    }
                    self.config.add_connection(config.clone());
                    let _ = self.cmd_tx.try_send(DbCommand::Connect { config });
                    self.status = if is_edit {
                        "Reconnecting with updated settings…".to_string()
                    } else {
                        "Connecting…".to_string()
                    };
                    close_dialog = true;
                }
            }
        }
        if close_dialog {
            self.connection_dialog = None;
        }
    }
}
