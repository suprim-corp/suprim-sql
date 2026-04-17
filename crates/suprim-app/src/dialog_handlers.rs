//! Dialog result handlers — connection, delete, and input dialogs.
//! Extracted from `app_ui.rs` to keep the main UI renderer focused.

use eframe::egui;
use suprim_core::db::commands::DbCommand;

use crate::app::App;

impl App {
    /// Shows the connection dialog modal and handles its result.
    pub(crate) fn render_connection_dialog(&mut self, ctx: &egui::Context) {
        let mut close_dialog = false;
        if let Some(dialog) = &mut self.connection_dialog {
            let result = dialog.show(ctx, &self.cmd_tx);
            match result {
                crate::ui::DialogResult::Pending => {}
                crate::ui::DialogResult::Cancelled => close_dialog = true,
                crate::ui::DialogResult::Confirmed(config) => {
                    let config = *config;
                    let is_edit = self.config.connections.iter().any(|c| c.id == config.id);

                    // ── License gate: check driver tier ─────────────────────
                    if let Err(msg) = self.gate.can_use_driver(&config.driver_type()) {
                        self.upgrade_prompt = Some(crate::ui::UpgradePrompt::new(&msg));
                        self.connection_dialog = None;
                        return;
                    }
                    // ── License gate: check connection limit (new only) ─────
                    if !is_edit {
                        if let Err(msg) =
                            self.gate.can_add_connection(self.config.connections.len())
                        {
                            self.upgrade_prompt = Some(crate::ui::UpgradePrompt::new(&msg));
                            self.connection_dialog = None;
                            return;
                        }
                    }

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
    pub(crate) fn render_delete_connection_dialog(&mut self, ctx: &egui::Context) {
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
    pub(crate) fn render_input_dialog(&mut self, ctx: &egui::Context) {
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

    /// Shows the account dialog and handles its result.
    pub(crate) fn render_license_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.license_dialog else {
            return;
        };
        match dialog.show(ctx) {
            crate::ui::LicenseDialogResult::Pending => {}
            crate::ui::LicenseDialogResult::SignIn { email, password } => {
                // TODO: wire to auth API when server is ready
                let _ = (email, password);
                self.status = "Sign in requires the auth server.".to_string();
                self.license_dialog = None;
            }
            crate::ui::LicenseDialogResult::SignOut => {
                // TODO: wire to gate.sign_out() when server is ready
                self.status = "Sign out requires the auth server.".to_string();
                self.license_dialog = None;
            }
            crate::ui::LicenseDialogResult::Cancelled => {
                self.license_dialog = None;
            }
        }
    }

    /// Shows the upgrade prompt dialog and handles its result.
    pub(crate) fn render_upgrade_prompt(&mut self, ctx: &egui::Context) {
        let Some(prompt) = &self.upgrade_prompt else {
            return;
        };
        match prompt.show(ctx) {
            crate::ui::UpgradePromptResult::Pending => {}
            crate::ui::UpgradePromptResult::OpenLicenseDialog => {
                self.upgrade_prompt = None;
                self.open_license_dialog();
            }
            crate::ui::UpgradePromptResult::Dismissed => {
                self.upgrade_prompt = None;
            }
        }
    }

    /// Shows the export dialog and handles its result.
    pub(crate) fn render_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = &mut self.export_dialog else {
            return;
        };
        match dialog.show(ctx) {
            crate::ui::ExportOutcome::Pending => {}
            crate::ui::ExportOutcome::Cancelled => {
                self.export_dialog = None;
            }
            crate::ui::ExportOutcome::Export(req) => {
                self.handle_export_request(req);
                self.export_dialog = None;
            }
        }
    }

    /// Perform an export — either write immediately (QueryResult mode) or
    /// enqueue DbCommand::Execute for each table (Tables mode).
    fn handle_export_request(&mut self, req: crate::ui::export::ExportRequest) {
        use crate::app::PendingExport;
        use crate::ui::export::{ExportFormatId, ExportModeKind};

        match req.mode_kind {
            ExportModeKind::QueryResult => {
                if let Some(result) = &req.query_result {
                    let res = match req.format {
                        ExportFormatId::Csv => crate::ui::export::csv_plugin::export(
                            result,
                            &req.destination,
                            &req.csv_options,
                        ),
                        ExportFormatId::Json => crate::ui::export::json_plugin::export(
                            result,
                            &req.destination,
                            &req.json_options,
                        ),
                    };
                    match res {
                        Ok(_) => {
                            self.status = format!(
                                "Exported {} rows to {}",
                                result.rows.len(),
                                req.destination.display()
                            );
                        }
                        Err(e) => {
                            self.status = format!("Export failed: {e}");
                            tracing::error!("Export failed: {e}");
                        }
                    }
                }
            }
            ExportModeKind::Tables => {
                let is_multi = req.selected_tables.len() > 1;
                for table in req.selected_tables {
                    let tab_id = uuid::Uuid::new_v4();
                    let sql = format!("SELECT * FROM \"{}\".\"{}\"", table.schema, table.name);
                    let dest = if is_multi {
                        req.destination
                            .join(format!("{}.{}", table.name, req.format.extension()))
                    } else {
                        req.destination.clone()
                    };
                    self.pending_exports.insert(
                        tab_id,
                        PendingExport {
                            destination: dest,
                            format: req.format,
                            csv_options: req.csv_options.clone(),
                            json_options: req.json_options.clone(),
                            table_name: table.name.clone(),
                        },
                    );
                    let _ = self.cmd_tx.try_send(DbCommand::Execute {
                        conn_id: table.conn_id,
                        tab_id,
                        sql,
                        database: Some(table.database),
                    });
                }
                self.status = "Exporting...".to_string();
            }
        }
    }
}
