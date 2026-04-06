use std::collections::HashMap;

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::db::driver::{DbCommand, DbEvent, DatabaseDriver};
use crate::db::factory::DbFactory;

// ─── DbWorker ────────────────────────────────────────────────────────────────

/// Asynchronous worker that owns all DB connections and processes commands
/// from the UI thread via a channel.
///
/// Run this on a dedicated `tokio::spawn` task. The UI sends `DbCommand`
/// values via the `cmd_tx` sender it receives from `DbWorker::spawn()`.
pub struct DbWorker {
    cmd_rx: mpsc::Receiver<DbCommand>,
    event_tx: mpsc::Sender<DbEvent>,
    /// Active connections keyed by conn_id
    connections: HashMap<Uuid, Box<dyn DatabaseDriver>>,
}

impl DbWorker {
    /// Create a new worker and return:
    /// - `cmd_tx`: UI side sends commands here
    /// - `event_rx`: UI side receives events from here
    /// - The worker itself (call `.run()` on a spawn)
    pub fn new(
        cmd_capacity: usize,
        event_capacity: usize,
    ) -> (mpsc::Sender<DbCommand>, mpsc::Receiver<DbEvent>, Self) {
        let (cmd_tx, cmd_rx) = mpsc::channel(cmd_capacity);
        let (event_tx, event_rx) = mpsc::channel(event_capacity);
        (
            cmd_tx,
            event_rx,
            Self {
                cmd_rx,
                event_tx,
                connections: HashMap::new(),
            },
        )
    }

    /// Convenience: spawn the worker onto a tokio task and return the channel ends.
    pub fn spawn(
        cmd_capacity: usize,
        event_capacity: usize,
    ) -> (mpsc::Sender<DbCommand>, mpsc::Receiver<DbEvent>) {
        let (cmd_tx, event_rx, worker) = Self::new(cmd_capacity, event_capacity);
        tokio::spawn(async move {
            worker.run().await;
        });
        (cmd_tx, event_rx)
    }

    /// Main event loop. Runs until the command channel is closed or `Shutdown` is received.
    pub async fn run(mut self) {
        while let Some(cmd) = self.cmd_rx.recv().await {
            match cmd {
                DbCommand::Shutdown => break,
                other => self.handle(other).await,
            }
        }
    }

    async fn handle(&mut self, cmd: DbCommand) {
        match cmd {
            DbCommand::Connect { config } => {
                let conn_id = config.id;
                match DbFactory::create(&config) {
                    Err(e) => {
                        let _ = self
                            .event_tx
                            .send(DbEvent::Error {
                                conn_id: Some(conn_id),
                                tab_id: None,
                                message: e.to_string(),
                            })
                            .await;
                    }
                    Ok(mut driver) => {
                        match driver.connect(&config).await {
                            Err(e) => {
                                let _ = self
                                    .event_tx
                                    .send(DbEvent::Error {
                                        conn_id: Some(conn_id),
                                        tab_id: None,
                                        message: e.to_string(),
                                    })
                                    .await;
                            }
                            Ok(()) => {
                                // List databases immediately after connect
                                let db_result = driver.list_databases().await;
                                self.connections.insert(conn_id, driver);
                                match db_result {
                                    Ok(databases) => {
                                        // Send ALL databases — sidebar handles
                                        // visible_databases filtering during render.
                                        let _ = self
                                            .event_tx
                                            .send(DbEvent::Connected { conn_id, databases })
                                            .await;
                                    }
                                    Err(e) => {
                                        let _ = self
                                            .event_tx
                                            .send(DbEvent::Error {
                                                conn_id: Some(conn_id),
                                                tab_id: None,
                                                message: format!(
                                                    "Connected but listing databases failed: {}",
                                                    e
                                                ),
                                            })
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            DbCommand::Disconnect { conn_id } => {
                if let Some(mut driver) = self.connections.remove(&conn_id) {
                    let _ = driver.disconnect().await;
                }
                let _ = self
                    .event_tx
                    .send(DbEvent::Disconnected { conn_id })
                    .await;
            }

            DbCommand::Execute { conn_id, tab_id, sql } => {
                match self.connections.get(&conn_id) {
                    None => {
                        let _ = self
                            .event_tx
                            .send(DbEvent::Error {
                                conn_id: Some(conn_id),
                                tab_id: Some(tab_id),
                                message: "Not connected".to_string(),
                            })
                            .await;
                    }
                    Some(driver) => match driver.execute(&sql).await {
                        Ok(result) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::QueryResult { tab_id, result })
                                .await;
                        }
                        Err(e) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::Error {
                                    conn_id: Some(conn_id),
                                    tab_id: Some(tab_id),
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    },
                }
            }

            DbCommand::ListDatabases { conn_id } => {
                match self.connections.get(&conn_id) {
                    None => {
                        let _ = self
                            .event_tx
                            .send(DbEvent::Error {
                                conn_id: Some(conn_id),
                                tab_id: None,
                                message: "Not connected".to_string(),
                            })
                            .await;
                    }
                    Some(driver) => match driver.list_databases().await {
                        Ok(databases) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::DatabasesListed { conn_id, databases })
                                .await;
                        }
                        Err(e) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::Error {
                                    conn_id: Some(conn_id),
                                    tab_id: None,
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    },
                }
            }

            DbCommand::ListSchemas { conn_id, database } => {
                match self.connections.get(&conn_id) {
                    None => {
                        let _ = self
                            .event_tx
                            .send(DbEvent::Error {
                                conn_id: Some(conn_id),
                                tab_id: None,
                                message: "Not connected".to_string(),
                            })
                            .await;
                    }
                    Some(driver) => match driver.list_schemas(&database).await {
                        Ok(schemas) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::SchemasListed {
                                    conn_id,
                                    database,
                                    schemas,
                                })
                                .await;
                        }
                        Err(e) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::Error {
                                    conn_id: Some(conn_id),
                                    tab_id: None,
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    },
                }
            }

            DbCommand::LoadSchemaDetail {
                conn_id,
                schema_name,
            } => {
                match self.connections.get(&conn_id) {
                    None => {
                        let _ = self
                            .event_tx
                            .send(DbEvent::Error {
                                conn_id: Some(conn_id),
                                tab_id: None,
                                message: "Not connected".to_string(),
                            })
                            .await;
                    }
                    Some(driver) => match driver.load_schema_detail(&schema_name).await {
                        Ok(schema_node) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::SchemaDetailLoaded {
                                    conn_id,
                                    schema_name,
                                    schema_node,
                                })
                                .await;
                        }
                        Err(e) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::Error {
                                    conn_id: Some(conn_id),
                                    tab_id: None,
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    },
                }
            }

            DbCommand::LoadTableData {
                conn_id,
                tab_id,
                schema,
                table,
                page,
                page_size,
            } => {
                match self.connections.get(&conn_id) {
                    None => {
                        let _ = self
                            .event_tx
                            .send(DbEvent::Error {
                                conn_id: Some(conn_id),
                                tab_id: Some(tab_id),
                                message: "Not connected".to_string(),
                            })
                            .await;
                    }
                    Some(driver) => {
                        match driver
                            .table_data(schema.as_deref(), &table, page, page_size)
                            .await
                        {
                            Ok(result) => {
                                let _ = self
                                    .event_tx
                                    .send(DbEvent::QueryResult { tab_id, result })
                                    .await;
                            }
                            Err(e) => {
                                let _ = self
                                    .event_tx
                                    .send(DbEvent::Error {
                                        conn_id: Some(conn_id),
                                        tab_id: Some(tab_id),
                                        message: e.to_string(),
                                    })
                                    .await;
                            }
                        }
                    }
                }
            }

            DbCommand::InsertRow {
                conn_id,
                tab_id,
                table,
                values,
            } => {
                match self.connections.get(&conn_id) {
                    None => {
                        let _ = self
                            .event_tx
                            .send(DbEvent::Error {
                                conn_id: Some(conn_id),
                                tab_id: Some(tab_id),
                                message: "Not connected".to_string(),
                            })
                            .await;
                    }
                    Some(driver) => match driver.insert_row(&table, values).await {
                        Ok(rows_affected) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::RowMutated { tab_id, rows_affected })
                                .await;
                        }
                        Err(e) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::Error {
                                    conn_id: Some(conn_id),
                                    tab_id: Some(tab_id),
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    },
                }
            }

            DbCommand::UpdateRow {
                conn_id,
                tab_id,
                table,
                pk,
                changes,
            } => {
                match self.connections.get(&conn_id) {
                    None => {
                        let _ = self
                            .event_tx
                            .send(DbEvent::Error {
                                conn_id: Some(conn_id),
                                tab_id: Some(tab_id),
                                message: "Not connected".to_string(),
                            })
                            .await;
                    }
                    Some(driver) => match driver.update_row(&table, pk, changes).await {
                        Ok(rows_affected) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::RowMutated { tab_id, rows_affected })
                                .await;
                        }
                        Err(e) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::Error {
                                    conn_id: Some(conn_id),
                                    tab_id: Some(tab_id),
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    },
                }
            }

            DbCommand::DeleteRow {
                conn_id,
                tab_id,
                table,
                pk,
            } => {
                match self.connections.get(&conn_id) {
                    None => {
                        let _ = self
                            .event_tx
                            .send(DbEvent::Error {
                                conn_id: Some(conn_id),
                                tab_id: Some(tab_id),
                                message: "Not connected".to_string(),
                            })
                            .await;
                    }
                    Some(driver) => match driver.delete_row(&table, pk).await {
                        Ok(rows_affected) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::RowMutated { tab_id, rows_affected })
                                .await;
                        }
                        Err(e) => {
                            let _ = self
                                .event_tx
                                .send(DbEvent::Error {
                                    conn_id: Some(conn_id),
                                    tab_id: Some(tab_id),
                                    message: e.to_string(),
                                })
                                .await;
                        }
                    },
                }
            }

            DbCommand::Shutdown => unreachable!("handled in run()"),
        }
    }
}

// ─── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::DriverParams;
    use crate::db::connection::ConnectionConfig;

    // ── Channel construction ──────────────────────────────────────────────────

    #[test]
    fn new_returns_channels_and_worker() {
        let (cmd_tx, _event_rx, _worker) = DbWorker::new(32, 32);
        // Can send a command without panic
        let _ = cmd_tx;
    }

    // ── Shutdown stops the run loop ───────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_stops_worker() {
        let (cmd_tx, _event_rx) = DbWorker::spawn(8, 8);
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
        // After shutdown the worker task exits; cmd_tx should still be valid
        // (drop would cause channel close but no panic)
        drop(cmd_tx);
    }

    // ── Execute on unknown conn_id → Error event ──────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn execute_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8);

        let conn_id = Uuid::new_v4();
        let tab_id = Uuid::new_v4();
        cmd_tx
            .send(DbCommand::Execute {
                conn_id,
                tab_id,
                sql: "SELECT 1".to_string(),
            })
            .await
            .unwrap();

        let event = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            event_rx.recv(),
        )
        .await
        .unwrap()
        .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }

    // ── ListDatabases on unknown conn → Error ────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn list_databases_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8);
        let conn_id = Uuid::new_v4();
        cmd_tx
            .send(DbCommand::ListDatabases { conn_id })
            .await
            .unwrap();

        let event = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            event_rx.recv(),
        )
        .await
        .unwrap()
        .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }

    // ── Insert/Update/Delete on unknown conn → Error ──────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn insert_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8);
        let conn_id = Uuid::new_v4();
        let tab_id = Uuid::new_v4();
        cmd_tx
            .send(DbCommand::InsertRow {
                conn_id,
                tab_id,
                table: "t".to_string(),
                values: HashMap::new(),
            })
            .await
            .unwrap();

        let event = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            event_rx.recv(),
        )
        .await
        .unwrap()
        .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn update_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8);
        let conn_id = Uuid::new_v4();
        let tab_id = Uuid::new_v4();
        cmd_tx
            .send(DbCommand::UpdateRow {
                conn_id,
                tab_id,
                table: "t".to_string(),
                pk: HashMap::new(),
                changes: HashMap::new(),
            })
            .await
            .unwrap();

        let event = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            event_rx.recv(),
        )
        .await
        .unwrap()
        .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn delete_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8);
        let conn_id = Uuid::new_v4();
        let tab_id = Uuid::new_v4();
        cmd_tx
            .send(DbCommand::DeleteRow {
                conn_id,
                tab_id,
                table: "t".to_string(),
                pk: HashMap::new(),
            })
            .await
            .unwrap();

        let event = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            event_rx.recv(),
        )
        .await
        .unwrap()
        .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn load_table_data_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8);
        let conn_id = Uuid::new_v4();
        let tab_id = Uuid::new_v4();
        cmd_tx
            .send(DbCommand::LoadTableData {
                conn_id,
                tab_id,
                schema: None,
                table: "users".to_string(),
                page: 0,
                page_size: 50,
            })
            .await
            .unwrap();

        let event = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            event_rx.recv(),
        )
        .await
        .unwrap()
        .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }

    // ── Connected-driver tests (require SQLite/MongoDB — commented out for postgres-only build) ──
    // TODO: Re-enable when adding SQLite driver back
    // Tests: connect_sqlite helper, execute_on_connected, list_databases_on_connected,
    //   load_table_data_on_connected, insert/update/delete on connected,
    //   disconnect_connected, execute_invalid_sql, connect_bad_sqlite_path,
    //   insert/update/delete_nonexistent_table, connect_mongodb_invalid_uri
}
