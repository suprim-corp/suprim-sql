use eframe::egui;
use suprim_sql::db::driver::{DbCommand, DbEvent};
use suprim_sql::storage::AppConfig;
use tokio::sync::mpsc;

use crate::sidebar_action_handler::{handle_sidebar_action, SidebarContext};
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

    /// Whether the About dialog is open.
    show_about: bool,

    /// Native macOS menu bar channel + retained handler objects.
    #[cfg(target_os = "macos")]
    native_menu: crate::ui::macos_menu::NativeMenu,
}

impl App {
    pub fn with_channels(
        cc: &eframe::CreationContext<'_>,
        cmd_tx: mpsc::Sender<DbCommand>,
        event_rx: mpsc::Receiver<DbEvent>,
        #[cfg(target_os = "macos")] native_menu: crate::ui::macos_menu::NativeMenu,
    ) -> Self {
        // Register Phosphor icon font so all UI components can use it.
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

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
                    "Reconnecting {} saved connection(s)\u{2026}",
                    config.connections.len()
                )
            },
            config,
            show_about: false,
            #[cfg(target_os = "macos")]
            native_menu,
        }
    }

    /// Drain all pending events from the DbWorker and update state.
    /// Returns `true` if at least one event was processed.
    fn process_events(&mut self) -> bool {
        let mut had_events = false;
        while let Ok(event) = self.event_rx.try_recv() {
            had_events = true;
            match event {
                DbEvent::Connected { conn_id, databases } => {
                    // Build a minimal SchemaTree from database names for sidebar display.
                    // Schemas will be loaded lazily via ListSchemas / LoadSchemaDetail.
                    let schema = suprim_sql::db::types::SchemaTree {
                        databases: databases
                            .into_iter()
                            .map(|name| suprim_sql::db::types::DatabaseNode {
                                id: uuid::Uuid::new_v4(),
                                name,
                                schemas: vec![],
                            })
                            .collect(),
                    };
                    // Use the saved connection name; fall back to conn_id string.
                    let saved = self.config.connections.iter().find(|c| c.id == conn_id);
                    let conn_name = saved
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| conn_id.to_string());
                    let visible_dbs = saved.and_then(|c| c.visible_databases.clone());
                    self.sidebar
                        .on_connected(conn_id, conn_name, schema, visible_dbs);
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
                    self.status =
                        format!("Query complete \u{2014} {row_count} rows  ({millis} ms)");
                }
                DbEvent::DatabasesListed { conn_id, databases } => {
                    // Rebuild schema tree from listed databases for sidebar.
                    let schema = suprim_sql::db::types::SchemaTree {
                        databases: databases
                            .into_iter()
                            .map(|name| suprim_sql::db::types::DatabaseNode {
                                id: uuid::Uuid::new_v4(),
                                name,
                                schemas: vec![],
                            })
                            .collect(),
                    };
                    self.sidebar.on_schema_loaded(conn_id, schema);
                }
                DbEvent::SchemasListed {
                    conn_id,
                    database,
                    schemas,
                } => {
                    self.sidebar.on_schemas_listed(conn_id, &database, schemas);
                }
                DbEvent::SchemaDetailLoaded {
                    conn_id,
                    database,
                    schema_name,
                    schema_node,
                } => {
                    self.sidebar.on_schema_detail_loaded(
                        conn_id,
                        &database,
                        &schema_name,
                        schema_node,
                    );
                }
                DbEvent::RowMutated {
                    tab_id,
                    rows_affected,
                } => {
                    self.tab_manager.on_row_mutated(tab_id, rows_affected);
                    self.status = format!("{rows_affected} row(s) affected");
                }
                DbEvent::DdlCompleted {
                    conn_id,
                    database,
                    schema_name,
                } => {
                    // Auto-refresh schema after DDL operations.
                    let _ = self.cmd_tx.try_send(DbCommand::LoadSchemaDetail {
                        conn_id,
                        database,
                        schema_name,
                    });
                    self.status = "Operation completed".to_string();
                }
                DbEvent::Error {
                    tab_id, message, ..
                } => {
                    if let Some(tid) = tab_id {
                        self.tab_manager.on_tab_error(tid);
                    }
                    self.status = format!("Error: {message}");
                }
            }
        }
        had_events
    }

    /// Process native macOS menu actions each frame.
    #[cfg(target_os = "macos")]
    fn process_menu_actions(&mut self, ctx: &egui::Context) {
        use crate::ui::macos_menu::MenuAction;

        while let Ok(action) = self.native_menu.rx.try_recv() {
            match action {
                MenuAction::NewConnection => {
                    self.connection_dialog = Some(ConnectionDialog::new());
                }
                MenuAction::Quit => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                MenuAction::NewSqlTab => {
                    if let Some((conn_id, name, database, databases)) =
                        self.sidebar.first_connection_info()
                    {
                        self.tab_manager
                            .open_sql_tab(Some(conn_id), name, database, databases);
                    } else {
                        self.tab_manager
                            .open_sql_tab(None, String::new(), None, Vec::new());
                    }
                }
                MenuAction::ReloadDatabases => {
                    for conn_id in self.sidebar.active_connection_ids() {
                        let _ = self.cmd_tx.try_send(DbCommand::ListDatabases { conn_id });
                    }
                }
            }
            ctx.request_repaint();
        }
    }
}

impl eframe::App for App {
    /// Process DB events (called before rendering each frame).
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let had_events = self.process_events();
        if had_events {
            // More events may arrive — repaint immediately.
            ctx.request_repaint();
        }

        // Poll native macOS menu actions.
        #[cfg(target_os = "macos")]
        self.process_menu_actions(ctx);

        // Poll for DB responses at 30fps while any tab is loading.
        // Otherwise, egui only repaints on user interaction (fully reactive).
        let any_loading = self.tab_manager.any_tab_loading();
        if any_loading {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // ── Custom title bar (macOS only) ───────────────────────────────
        #[cfg(target_os = "macos")]
        {
            use crate::ui::custom_title_bar::{self, TitleBarAction};
            let title_action = custom_title_bar::show_title_bar(ui);
            if title_action == TitleBarAction::AboutClicked {
                self.show_about = true;
            }
        }

        // ── Top menu bar (non-macOS only; macOS uses native system menu) ──
        #[cfg(not(target_os = "macos"))]
        egui::Panel::top("menu_bar").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Connection", |ui| {
                    if ui
                        .button("New Connection\u{2026}")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.connection_dialog = Some(ConnectionDialog::new());
                        ui.close();
                    }
                    ui.separator();
                    if ui
                        .button("Quit")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("Query", |ui| {
                    if ui
                        .button("New SQL Tab")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        if let Some((conn_id, name, database, databases)) =
                            self.sidebar.first_connection_info()
                        {
                            self.tab_manager
                                .open_sql_tab(Some(conn_id), name, database, databases);
                        } else {
                            self.tab_manager
                                .open_sql_tab(None, String::new(), None, Vec::new());
                        }
                        ui.close();
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui
                        .button("Reload Databases")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        for conn_id in self.sidebar.active_connection_ids() {
                            let _ = self.cmd_tx.try_send(DbCommand::ListDatabases { conn_id });
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
                    let sidebar = &self.sidebar;
                    let mut ctx = SidebarContext {
                        cmd_tx: &self.cmd_tx,
                        tab_manager: &mut self.tab_manager,
                        config: &mut self.config,
                        connection_dialog: &mut self.connection_dialog,
                        conn_name: Box::new(|id| sidebar.conn_name(id)),
                    };
                    handle_sidebar_action(act, &mut ctx);
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
                        "Reconnecting with updated settings\u{2026}".to_string()
                    } else {
                        "Connecting\u{2026}".to_string()
                    };
                    close_dialog = true;
                }
            }
        }
        if close_dialog {
            self.connection_dialog = None;
        }

        // ── About dialog (modal) ────────────────────────────────────────
        if self.show_about {
            self.show_about = crate::ui::about_dialog::show_about_dialog(&ctx);
        }
    }
}
