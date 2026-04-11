/// Command dispatch — routes each `DbCommand` variant to the appropriate handler.
/// Individual handlers live in sibling modules grouped by concern:
/// - `handle_connection.rs` — connect / disconnect
/// - `handle_query.rs` — execute, list databases/schemas, load schema detail, load table data
/// - `handle_mutation.rs` — insert / update / delete row
use crate::db::driver::DbCommand;

use super::DbWorker;

impl DbWorker {
    /// Dispatch a single command to the appropriate handler.
    pub(super) async fn handle(&mut self, cmd: DbCommand) {
        match cmd {
            DbCommand::Connect { config } => self.handle_connect(config).await,
            DbCommand::TestConnection { config } => self.handle_test_connection(config).await,
            DbCommand::Disconnect { conn_id } => self.handle_disconnect(conn_id).await,
            DbCommand::Execute {
                conn_id,
                tab_id,
                sql,
                database,
            } => {
                self.handle_execute(conn_id, tab_id, &sql, database.as_deref())
                    .await
            }
            DbCommand::ListDatabases { conn_id } => self.handle_list_databases(conn_id).await,
            DbCommand::ListSchemas { conn_id, database } => {
                self.handle_list_schemas(conn_id, &database).await
            }
            DbCommand::LoadSchemaDetail {
                conn_id,
                database,
                schema_name,
            } => {
                self.handle_load_schema_detail(conn_id, &database, &schema_name)
                    .await
            }
            DbCommand::LoadTableData {
                conn_id,
                tab_id,
                database,
                schema,
                table,
                page,
                page_size,
                where_clause,
                order_clause,
            } => {
                self.handle_load_table_data(
                    conn_id,
                    tab_id,
                    database.as_deref(),
                    schema.as_deref(),
                    &table,
                    page,
                    page_size,
                    where_clause.as_deref(),
                    order_clause.as_deref(),
                )
                .await
            }
            DbCommand::InsertRow {
                conn_id,
                tab_id,
                table,
                values,
            } => self.handle_insert_row(conn_id, tab_id, &table, values).await,
            DbCommand::UpdateRow {
                conn_id,
                tab_id,
                table,
                pk,
                changes,
            } => {
                self.handle_update_row(conn_id, tab_id, &table, pk, changes)
                    .await
            }
            DbCommand::DeleteRow {
                conn_id,
                tab_id,
                table,
                pk,
            } => self.handle_delete_row(conn_id, tab_id, &table, pk).await,

            // ── DDL commands (use handle_ddl helper with inline closure) ──────
            DbCommand::TruncateTable {
                conn_id,
                database,
                schema_name,
                table_name,
            } => {
                self.handle_ddl(conn_id, &database, &schema_name, async {
                    self.connections
                        .get(&conn_id)
                        .ok_or_else(|| crate::error::AppError::NotConnected)?
                        .truncate_table(&schema_name, &table_name)
                        .await
                })
                .await
            }
            DbCommand::DropTable {
                conn_id,
                database,
                schema_name,
                table_name,
            } => {
                self.handle_ddl(conn_id, &database, &schema_name, async {
                    self.connections
                        .get(&conn_id)
                        .ok_or_else(|| crate::error::AppError::NotConnected)?
                        .drop_table(&schema_name, &table_name)
                        .await
                })
                .await
            }
            DbCommand::DropView {
                conn_id,
                database,
                schema_name,
                view_name,
            } => {
                self.handle_ddl(conn_id, &database, &schema_name, async {
                    self.connections
                        .get(&conn_id)
                        .ok_or_else(|| crate::error::AppError::NotConnected)?
                        .drop_view(&schema_name, &view_name)
                        .await
                })
                .await
            }
            DbCommand::RenameTable {
                conn_id,
                database,
                schema_name,
                old_name,
                new_name,
            } => {
                self.handle_ddl(conn_id, &database, &schema_name, async {
                    self.connections
                        .get(&conn_id)
                        .ok_or_else(|| crate::error::AppError::NotConnected)?
                        .rename_table(&schema_name, &old_name, &new_name)
                        .await
                })
                .await
            }
            DbCommand::CreateDatabase { conn_id, name } => {
                self.handle_create_database(conn_id, &name).await
            }
            DbCommand::CreateSchema {
                conn_id,
                database,
                name,
            } => {
                self.handle_create_schema(conn_id, &database, &name).await
            }
            DbCommand::CompareSchemas {
                source_conn_id,
                source_database,
                source_schema,
                target_conn_id,
                target_database,
                target_schema,
            } => {
                self.handle_compare_schemas(
                    source_conn_id,
                    &source_database,
                    &source_schema,
                    target_conn_id,
                    &target_database,
                    &target_schema,
                )
                .await
            }
            DbCommand::Shutdown => unreachable!("handled in run()"),
        }
    }
}
