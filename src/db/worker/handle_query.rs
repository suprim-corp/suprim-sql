/// Read-only query handlers: execute, list databases/schemas, load schema detail, load table data.
use uuid::Uuid;

use crate::db::driver::DbEvent;

use super::DbWorker;

impl DbWorker {
    pub(super) async fn handle_execute(&self, conn_id: Uuid, tab_id: Uuid, sql: &str) {
        if let Some(driver) = self.get_driver(conn_id, Some(tab_id)).await {
            match driver.execute(sql).await {
                Ok(result) => {
                    let _ = self
                        .event_tx
                        .send(DbEvent::QueryResult { tab_id, result })
                        .await;
                }
                Err(e) => {
                    self.send_error(conn_id, Some(tab_id), e.to_string()).await;
                }
            }
        }
    }

    pub(super) async fn handle_list_databases(&self, conn_id: Uuid) {
        if let Some(driver) = self.get_driver(conn_id, None).await {
            match driver.list_databases().await {
                Ok(databases) => {
                    let _ = self
                        .event_tx
                        .send(DbEvent::DatabasesListed { conn_id, databases })
                        .await;
                }
                Err(e) => self.send_error(conn_id, None, e.to_string()).await,
            }
        }
    }

    pub(super) async fn handle_list_schemas(&self, conn_id: Uuid, database: &str) {
        if let Some(driver) = self.get_driver(conn_id, None).await {
            match driver.list_schemas(database).await {
                Ok(schemas) => {
                    let _ = self
                        .event_tx
                        .send(DbEvent::SchemasListed {
                            conn_id,
                            database: database.to_string(),
                            schemas,
                        })
                        .await;
                }
                Err(e) => self.send_error(conn_id, None, e.to_string()).await,
            }
        }
    }

    pub(super) async fn handle_load_schema_detail(
        &self,
        conn_id: Uuid,
        database: &str,
        schema_name: &str,
    ) {
        if let Some(driver) = self.get_driver(conn_id, None).await {
            match driver.load_schema_detail(database, schema_name).await {
                Ok(schema_node) => {
                    let _ = self
                        .event_tx
                        .send(DbEvent::SchemaDetailLoaded {
                            conn_id,
                            database: database.to_string(),
                            schema_name: schema_name.to_string(),
                            schema_node,
                        })
                        .await;
                }
                Err(e) => self.send_error(conn_id, None, e.to_string()).await,
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn handle_load_table_data(
        &self,
        conn_id: Uuid,
        tab_id: Uuid,
        database: Option<&str>,
        schema: Option<&str>,
        table: &str,
        page: u32,
        page_size: u32,
        where_clause: Option<&str>,
        order_clause: Option<&str>,
    ) {
        if let Some(driver) = self.get_driver(conn_id, Some(tab_id)).await {
            match driver
                .table_data(
                    database,
                    schema,
                    table,
                    page,
                    page_size,
                    where_clause,
                    order_clause,
                )
                .await
            {
                Ok(result) => {
                    let _ = self
                        .event_tx
                        .send(DbEvent::QueryResult { tab_id, result })
                        .await;
                }
                Err(e) => {
                    self.send_error(conn_id, Some(tab_id), e.to_string()).await;
                }
            }
        }
    }
}
