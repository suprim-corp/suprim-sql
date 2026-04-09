/// Handles `SidebarAction` dispatches — maps sidebar UI actions to DbCommands
/// or application state changes. Extracted from `app.rs` to isolate sidebar
/// action routing from core application wiring.
use suprim_sql::db::driver::DbCommand;
use suprim_sql::storage::AppConfig;
use tokio::sync::mpsc;

use crate::ui::{ConnectionDialog, DeleteConnectionDialog, SidebarAction, TabManager};

/// Sidebar context passed to the handler so it can mutate application state
/// without needing a full `&mut App`.
pub struct SidebarContext<'a> {
    pub cmd_tx: &'a mpsc::Sender<DbCommand>,
    pub tab_manager: &'a mut TabManager,
    pub config: &'a mut AppConfig,
    pub connection_dialog: &'a mut Option<ConnectionDialog>,
    pub delete_connection_dialog: &'a mut Option<DeleteConnectionDialog>,
    pub pending_delete_conn: &'a mut Option<uuid::Uuid>,
    /// Closure to look up a connection name by id (delegates to sidebar).
    pub conn_name: Box<dyn Fn(uuid::Uuid) -> String + 'a>,
}

/// Dispatch a single `SidebarAction` — translating it into the appropriate
/// DbCommand send or state mutation.
pub fn handle_sidebar_action(action: SidebarAction, ctx: &mut SidebarContext<'_>) {
    match action {
        // Connect is handled directly in app_ui.rs (needs sidebar mutation).
        SidebarAction::Connect { .. } => {}
        SidebarAction::NewConnection => {
            *ctx.connection_dialog = Some(ConnectionDialog::new());
        }
        SidebarAction::EditConnection { conn_id } => {
            if let Some(cfg) = ctx.config.connections.iter().find(|c| c.id == conn_id) {
                *ctx.connection_dialog = Some(ConnectionDialog::from_config(cfg));
            }
        }
        SidebarAction::OpenSqlTab {
            conn_id,
            database,
            databases,
        } => {
            let name = (ctx.conn_name)(conn_id);
            ctx.tab_manager
                .open_sql_tab(Some(conn_id), name, database, databases);
        }
        SidebarAction::OpenTableViewer {
            conn_id,
            database,
            schema_name,
            table_name,
        } => {
            let name = (ctx.conn_name)(conn_id);
            ctx.tab_manager
                .open_table_viewer(conn_id, name, database, schema_name, table_name);
        }
        SidebarAction::EditTable {
            conn_id,
            database,
            schema_name,
            table,
        } => {
            let name = (ctx.conn_name)(conn_id);
            ctx.tab_manager
                .open_table_editor(conn_id, name, database, schema_name, &table);
        }
        SidebarAction::Disconnect { conn_id } => {
            let _ = ctx.cmd_tx.try_send(DbCommand::Disconnect { conn_id });
        }
        SidebarAction::DeleteConnection { conn_id, conn_name } => {
            *ctx.delete_connection_dialog = Some(DeleteConnectionDialog::new(&conn_name));
            *ctx.pending_delete_conn = Some(conn_id);
        }
        SidebarAction::LoadSchemaDetail {
            conn_id,
            database,
            schema_name,
        } => {
            let _ = ctx.cmd_tx.try_send(DbCommand::LoadSchemaDetail {
                conn_id,
                database,
                schema_name,
            });
        }
        SidebarAction::ListSchemas { conn_id, database } => {
            let _ = ctx
                .cmd_tx
                .try_send(DbCommand::ListSchemas { conn_id, database });
        }
        SidebarAction::UpdateVisibleDatabases { conn_id, visible } => {
            if let Some(cfg) = ctx.config.connections.iter_mut().find(|c| c.id == conn_id) {
                cfg.visible_databases = visible;
                ctx.config.save();
            }
            let _ = ctx.cmd_tx.try_send(DbCommand::ListDatabases { conn_id });
        }
        SidebarAction::RefreshSchema {
            conn_id,
            database,
            schema_name,
        } => {
            let _ = ctx.cmd_tx.try_send(DbCommand::LoadSchemaDetail {
                conn_id,
                database,
                schema_name,
            });
        }
        SidebarAction::TruncateTable {
            conn_id,
            database,
            schema_name,
            table_name,
        } => {
            let _ = ctx.cmd_tx.try_send(DbCommand::TruncateTable {
                conn_id,
                database,
                schema_name,
                table_name,
            });
        }
        SidebarAction::DropTable {
            conn_id,
            database,
            schema_name,
            table_name,
        } => {
            let _ = ctx.cmd_tx.try_send(DbCommand::DropTable {
                conn_id,
                database,
                schema_name,
                table_name,
            });
        }
        SidebarAction::DropView {
            conn_id,
            database,
            schema_name,
            view_name,
        } => {
            let _ = ctx.cmd_tx.try_send(DbCommand::DropView {
                conn_id,
                database,
                schema_name,
                view_name,
            });
        }
        SidebarAction::RenameTable {
            conn_id,
            database,
            schema_name,
            old_name,
            new_name,
        } => {
            let _ = ctx.cmd_tx.try_send(DbCommand::RenameTable {
                conn_id,
                database,
                schema_name,
                old_name,
                new_name,
            });
        }
    }
}
