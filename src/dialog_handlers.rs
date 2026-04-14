//! Dialog result handlers — connection, delete, and input dialogs.
//! Extracted from `app_ui.rs` to keep the main UI renderer focused.

use eframe::egui;
use suprim_sql::db::commands::DbCommand;

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
}
