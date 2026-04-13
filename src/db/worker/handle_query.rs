/// Read-only query handlers: execute, list databases/schemas, load schema detail, load table data.
use uuid::Uuid;

use crate::db::driver::DbEvent;

use super::DbWorker;

impl DbWorker {
    pub(super) async fn handle_execute(
        &self,
        conn_id: Uuid,
        tab_id: Uuid,
        sql: &str,
        database: Option<&str>,
    ) {
        if let Some(driver) = self.get_driver(conn_id, Some(tab_id)).await {
            let result = match database {
                Some(db) => driver.execute_on_database(sql, db).await,
                None => driver.execute(sql).await,
            };
            match result {
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

    pub(super) async fn handle_compare_schemas(
        &self,
        source_conn_id: Uuid,
        source_database: &str,
        source_schema: &str,
        target_conn_id: Uuid,
        target_database: &str,
        target_schema: &str,
    ) {
        // Load source schema
        let src_driver = match self.connections.get(&source_conn_id) {
            Some(d) => d,
            None => {
                self.send_error(source_conn_id, None, "Source connection not found".into())
                    .await;
                return;
            }
        };
        let source_node = match src_driver
            .load_schema_detail(source_database, source_schema)
            .await
        {
            Ok(n) => n,
            Err(e) => {
                self.send_error(
                    source_conn_id,
                    None,
                    format!("Failed to load source schema: {e}"),
                )
                .await;
                return;
            }
        };
        let source_extensions = src_driver
            .list_extensions(source_database)
            .await
            .unwrap_or_default();

        // Load target schema
        let tgt_driver = match self.connections.get(&target_conn_id) {
            Some(d) => d,
            None => {
                self.send_error(target_conn_id, None, "Target connection not found".into())
                    .await;
                return;
            }
        };
        let target_node = match tgt_driver
            .load_schema_detail(target_database, target_schema)
            .await
        {
            Ok(n) => n,
            Err(e) => {
                self.send_error(
                    target_conn_id,
                    None,
                    format!("Failed to load target schema: {e}"),
                )
                .await;
                return;
            }
        };
        let target_extensions = tgt_driver
            .list_extensions(target_database)
            .await
            .unwrap_or_default();

        let _ = self
            .event_tx
            .send(DbEvent::SchemasCompared {
                source: source_node,
                target: target_node,
                source_extensions,
                target_extensions,
            })
            .await;
    }

    pub(super) async fn handle_create_database(&self, conn_id: Uuid, name: &str) {
        let driver = match self.connections.get(&conn_id) {
            Some(d) => d,
            None => {
                self.send_error(conn_id, None, "Not connected".into()).await;
                return;
            }
        };
        match driver.create_database(name).await {
            Ok(()) => {
                let _ = self
                    .event_tx
                    .send(DbEvent::DatabaseCreated { conn_id })
                    .await;
            }
            Err(e) => {
                self.send_error(conn_id, None, format!("Failed to create database: {e}"))
                    .await;
            }
        }
    }

    pub(super) async fn handle_create_schema(
        &self,
        conn_id: Uuid,
        database: &str,
        name: &str,
    ) {
        let driver = match self.connections.get(&conn_id) {
            Some(d) => d,
            None => {
                self.send_error(conn_id, None, "Not connected".into()).await;
                return;
            }
        };
        match driver.create_schema(database, name).await {
            Ok(()) => {
                let _ = self
                    .event_tx
                    .send(DbEvent::SchemaCreated {
                        conn_id,
                        database: database.to_string(),
                    })
                    .await;
            }
            Err(e) => {
                self.send_error(conn_id, None, format!("Failed to create schema: {e}"))
                    .await;
            }
        }
    }

    pub(super) async fn handle_load_dashboard(&self, conn_id: Uuid) {
        let driver = match self.connections.get(&conn_id) {
            Some(d) => d,
            None => {
                self.send_error(conn_id, None, "Not connected".into()).await;
                return;
            }
        };
        let sessions = driver.list_sessions().await.unwrap_or_default();
        let metrics = driver.server_metrics().await.unwrap_or_default();
        let _ = self
            .event_tx
            .send(DbEvent::DashboardLoaded {
                conn_id,
                sessions,
                metrics,
            })
            .await;
    }

    pub(super) async fn handle_kill_session(&self, conn_id: Uuid, pid: i32) {
        let driver = match self.connections.get(&conn_id) {
            Some(d) => d,
            None => {
                self.send_error(conn_id, None, "Not connected".into()).await;
                return;
            }
        };
        match driver.kill_session(pid).await {
            Ok(()) => {
                let _ = self
                    .event_tx
                    .send(DbEvent::SessionKilled { conn_id, pid })
                    .await;
            }
            Err(e) => {
                self.send_error(conn_id, None, format!("Failed to kill session: {e}"))
                    .await;
            }
        }
    }
}
