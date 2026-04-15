/// Full-frame UI rendering for the application.
/// Extracted from `app.rs` so the main file only holds struct definition,
/// construction, and the thin `eframe::App` trait glue.
use eframe::egui;

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
            let tier_name = self.gate.tier_name();
            let title_action = custom_title_bar::show_title_bar(ui, tier_name);
            match title_action {
                TitleBarAction::AboutClicked => {
                    self.show_about = true;
                }
                TitleBarAction::LicenseClicked => {
                    self.open_license_dialog();
                }
                _ => {}
            }
        }

        // ── Top menu bar (non-macOS only; macOS uses native system menu) ──
        #[cfg(not(target_os = "macos"))]
        self.render_menu_bar(ui, &ctx);

        // ── Status bar (bottom) ─────────────────────────────────────────
        let tier_name = self.gate.tier_name().to_string();
        egui::Panel::bottom("status_bar").show_inside(ui, |ui| {
            self.statusbar.show(ui, &self.status, &tier_name);
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
                let conn_limit = self.gate.connection_limit();
                let action = self.sidebar.show(ui, conn_limit);
                if let Some(act) = action {
                    // Handle Connect specially — needs sidebar mutation.
                    if let SidebarAction::Connect { conn_id } = &act {
                        let conn_id = *conn_id;
                        if let Some(cfg) = self.config.connections.iter().find(|c| c.id == conn_id)
                        {
                            self.sidebar.on_connecting(conn_id);
                            let _ = self.cmd_tx.try_send(
                                suprim_core::db::commands::DbCommand::Connect {
                                    config: cfg.clone(),
                                },
                            );
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
                            upgrade_prompt: &mut self.upgrade_prompt,
                            gate: self.gate.as_ref(),
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

        // ── License activation dialog (modal) ────────────────────────────
        self.render_license_dialog(&ctx);

        // ── Upgrade prompt dialog (modal) ────────────────────────────────
        self.render_upgrade_prompt(&ctx);

        // ── Structure Sync dialog (modal) ───────────────────────────────
        if let Some(dialog) = &mut self.structure_sync_dialog {
            let result = dialog.show(&ctx);
            // Connect disconnected connections first
            for conn_id in &result.connect_requests {
                if let Some(cfg) = self.config.connections.iter().find(|c| c.id == *conn_id) {
                    self.sidebar.on_connecting(*conn_id);
                    let _ = self
                        .cmd_tx
                        .try_send(suprim_core::db::commands::DbCommand::Connect {
                            config: cfg.clone(),
                        });
                }
            }
            // Send database list requests
            for conn_id in &result.database_requests {
                let _ = self
                    .cmd_tx
                    .try_send(suprim_core::db::commands::DbCommand::ListDatabases {
                        conn_id: *conn_id,
                    });
            }
            // Send schema load requests
            for (conn_id, database) in &result.schema_requests {
                let _ = self
                    .cmd_tx
                    .try_send(suprim_core::db::commands::DbCommand::ListSchemas {
                        conn_id: *conn_id,
                        database: database.clone(),
                    });
            }
            // Send compare request
            if let Some(req) = result.compare_request {
                let _ =
                    self.cmd_tx
                        .try_send(suprim_core::db::commands::DbCommand::CompareSchemas {
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
                        if let Err(msg) =
                            self.gate.can_add_connection(self.config.connections.len())
                        {
                            self.upgrade_prompt = Some(crate::ui::UpgradePrompt::new(&msg));
                        } else {
                            self.connection_dialog = Some(crate::ui::ConnectionDialog::new());
                        }
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
                            let _ = self.cmd_tx.try_send(
                                suprim_core::db::commands::DbCommand::ListDatabases { conn_id },
                            );
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
                ui.menu_button("License", |ui| {
                    if ui
                        .button("License\u{2026}")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.open_license_dialog();
                        ui.close();
                    }
                });
            });
        });
    }
}
