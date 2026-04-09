/// Full-frame UI rendering for the application.
/// Extracted from `app.rs` so the main file only holds struct definition,
/// construction, and the thin `eframe::App` trait glue.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;

use crate::app::App;
use crate::sidebar_action_handler::{handle_sidebar_action, SidebarContext};

impl App {
    /// Renders the entire application UI for one frame.
    /// Called from `eframe::App::ui()`.
    pub(crate) fn render_ui(&mut self, ui: &mut egui::Ui) {
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
        self.render_menu_bar(ui, &ctx);

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
        self.render_connection_dialog(&ctx);

        // ── About dialog (modal) ────────────────────────────────────────
        if self.show_about {
            self.show_about = crate::ui::about_dialog::show_about_dialog(&ctx);
        }

        // ── Structure Sync dialog (modal) ───────────────────────────────
        if let Some(dialog) = &mut self.structure_sync_dialog {
            if !dialog.show(&ctx) {
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
                });
            });
        });
    }

    /// Shows the connection dialog modal and handles its result.
    fn render_connection_dialog(&mut self, ctx: &egui::Context) {
        let mut close_dialog = false;
        if let Some(dialog) = &mut self.connection_dialog {
            let result = dialog.show(ctx);
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
}
