/// Full-frame UI rendering for the application.
/// Extracted from `app.rs` so the main file only holds struct definition,
/// construction, and the thin `eframe::App` trait glue.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;

use crate::app::App;
use crate::sidebar_action_handler::{handle_sidebar_action, SidebarContext};
use crate::ui::SidebarAction;

impl App {
    /// Renders the entire application UI for one frame.
    /// Called from `eframe::App::ui()`.
    pub(crate) fn render_ui(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();

        // ── Global keyboard shortcuts ───────────────────────────────────
        let toggle_history = ui.input(|i| i.key_pressed(egui::Key::Y) && i.modifiers.command);
        if toggle_history {
            self.show_history = !self.show_history;
        }

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
        self.render_menu_bar(ui, &ctx);

        // ── Status bar (bottom) ─────────────────────────────────────────
        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            self.statusbar.show(ui, &self.status);
        });

        // ── Query History panel (bottom, above status bar) ──────────────
        if self.show_history {
            egui::Panel::bottom("history_panel")
                .resizable(true)
                .default_size(200.0)
                .min_size(120.0)
                .max_size(400.0)
                .show_inside(ui, |ui| {
                    let output = crate::ui::query_history::render_history_panel(
                        ui,
                        &mut self.history,
                        &mut self.history_search,
                    );
                    if let Some(sql) = output.load_sql {
                        self.tab_manager.load_sql_into_active_editor(&sql);
                        self.show_history = false;
                    }
                    if output.clear_all {
                        self.history.clear();
                    }
                    if output.close {
                        self.show_history = false;
                    }
                });
        }

        // ── Sidebar (left) ──────────────────────────────────────────────
        egui::Panel::left("sidebar")
            .resizable(true)
            .default_size(220.0)
            .min_size(160.0)
            .show_inside(ui, |ui| {
                let action = self.sidebar.show(ui);
                if let Some(act) = action {
                    // Handle Connect specially — needs sidebar mutation.
                    if let SidebarAction::Connect { conn_id } = &act {
                        let conn_id = *conn_id;
                        if let Some(cfg) = self.config.connections.iter().find(|c| c.id == conn_id)
                        {
                            self.sidebar.on_connecting(conn_id);
                            let _ =
                                self.cmd_tx
                                    .try_send(suprim_sql::db::driver::DbCommand::Connect {
                                        config: cfg.clone(),
                                    });
                        }
                    } else {
                        let sidebar = &self.sidebar;
                        let mut ctx = SidebarContext {
                            cmd_tx: &self.cmd_tx,
                            tab_manager: &mut self.tab_manager,
                            config: &mut self.config,
                            connection_dialog: &mut self.connection_dialog,
                            delete_connection_dialog: &mut self.delete_connection_dialog,
                            pending_delete_conn: &mut self.pending_delete_conn,
                            input_dialog: &mut self.input_dialog,
                            conn_name: Box::new(|id| sidebar.conn_name(id)),
                        };
                        handle_sidebar_action(act, &mut ctx);
                    }
                }
            });

        // ── Main content area ───────────────────────────────────────────
        egui::CentralPanel::default().show_inside(ui, |ui| {
            self.tab_manager.show(ui, &self.cmd_tx);
        });

        // ── Connection dialog (modal) ───────────────────────────────────
        self.render_connection_dialog(&ctx);

        // ── About dialog (modal) ────────────────────────────────────────
        if self.show_about {
            self.show_about = crate::ui::about_dialog::show_about_dialog(&ctx);
        }

        // ── Delete connection dialog (modal) ─────────────────────────────
        self.render_delete_connection_dialog(&ctx);

        // ── Input dialog (New Database / New Schema) ─────────────────────
        self.render_input_dialog(&ctx);

        // ── Structure Sync dialog (modal) ───────────────────────────────
        if let Some(dialog) = &mut self.structure_sync_dialog {
            let result = dialog.show(&ctx);
            // Connect disconnected connections first
            for conn_id in &result.connect_requests {
                if let Some(cfg) = self.config.connections.iter().find(|c| c.id == *conn_id) {
                    self.sidebar.on_connecting(*conn_id);
                    let _ = self
                        .cmd_tx
                        .try_send(suprim_sql::db::driver::DbCommand::Connect {
                            config: cfg.clone(),
                        });
                }
            }
            // Send database list requests
            for conn_id in &result.database_requests {
                let _ = self
                    .cmd_tx
                    .try_send(suprim_sql::db::driver::DbCommand::ListDatabases {
                        conn_id: *conn_id,
                    });
            }
            // Send schema load requests
            for (conn_id, database) in &result.schema_requests {
                let _ = self
                    .cmd_tx
                    .try_send(suprim_sql::db::driver::DbCommand::ListSchemas {
                        conn_id: *conn_id,
                        database: database.clone(),
                    });
            }
            // Send compare request
            if let Some(req) = result.compare_request {
                let _ = self
                    .cmd_tx
                    .try_send(suprim_sql::db::driver::DbCommand::CompareSchemas {
                        source_conn_id: req.source_conn_id,
                        source_database: req.source_database,
                        source_schema: req.source_schema,
                        target_conn_id: req.target_conn_id,
                        target_database: req.target_database,
                        target_schema: req.target_schema,
                    });
            }
            if !result.open {
                self.structure_sync_dialog = None;
            }
        }
    }

    /// Renders the in-app menu bar (used on non-macOS platforms).
    #[cfg(not(target_os = "macos"))]
    fn render_menu_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        egui::Panel::top("menu_bar").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("Connection", |ui| {
                    if ui
                        .button("New Connection\u{2026}")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.connection_dialog = Some(crate::ui::ConnectionDialog::new());
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
                    if ui
                        .button("Query History")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.show_history = !self.show_history;
                        ui.close();
                    }
                });
            });
        });
    }

    /// Shows the connection dialog modal and handles its result.
    fn render_connection_dialog(&mut self, ctx: &egui::Context) {
        let mut close_dialog = false;
        if let Some(dialog) = &mut self.connection_dialog {
            let result = dialog.show(ctx, &self.cmd_tx);
            match result {
                crate::ui::DialogResult::Pending => {}
                crate::ui::DialogResult::Cancelled => close_dialog = true,
                crate::ui::DialogResult::Confirmed(config) => {
                    let is_edit = self.config.connections.iter().any(|c| c.id == config.id);
                    if is_edit {
                        let _ = self
                            .cmd_tx
                            .try_send(DbCommand::Disconnect { conn_id: config.id });
                    }
                    self.config.add_connection(config.clone());
                    // Ensure sidebar has an entry for this connection
                    self.sidebar.init_from_config(&self.config.connections);
                    self.sidebar.on_connecting(config.id);
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
    }

    /// Shows the delete connection dialog and handles its result.
    fn render_delete_connection_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &self.delete_connection_dialog else {
            return;
        };
        match dialog.show(ctx) {
            crate::ui::DeleteConnectionResult::Pending => {}
            crate::ui::DeleteConnectionResult::Confirmed => {
                // Execute the pending delete
                if let Some(conn_id) = self.pending_delete_conn.take() {
                    // Disconnect first (if connected, this is a no-op if already disconnected)
                    let _ = self.cmd_tx.try_send(DbCommand::Disconnect { conn_id });
                    // Remove from persistent config
                    self.config.remove_connection(conn_id);
                    // Remove from sidebar
                    self.sidebar.remove_connection(conn_id);
                    // Close any tabs associated with this connection
                    self.tab_manager.close_tabs_for_connection(conn_id);
                    self.status = "Connection deleted".to_string();
                }
                self.delete_connection_dialog = None;
            }
            crate::ui::DeleteConnectionResult::Cancelled => {
                self.pending_delete_conn = None;
                self.delete_connection_dialog = None;
            }
        }
    }

    /// Shows the input dialog (New Database / New Schema) and handles its result.
    fn render_input_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.input_dialog else {
            return;
        };
        match dialog.show(ctx) {
            crate::ui::InputDialogResult::Pending => {}
            crate::ui::InputDialogResult::Confirmed(name) => {
                match dialog.kind.clone() {
                    crate::ui::InputDialogKind::NewDatabase { conn_id } => {
                        let _ = self
                            .cmd_tx
                            .try_send(DbCommand::CreateDatabase { conn_id, name });
                    }
                    crate::ui::InputDialogKind::NewSchema { conn_id, database } => {
                        let _ = self.cmd_tx.try_send(DbCommand::CreateSchema {
                            conn_id,
                            database,
                            name,
                        });
                    }
                }
                self.input_dialog = None;
            }
            crate::ui::InputDialogResult::Cancelled => {
                self.input_dialog = None;
            }
        }
    }
}
