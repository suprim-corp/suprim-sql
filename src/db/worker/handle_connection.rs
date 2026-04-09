/// Connection lifecycle handlers: connect and disconnect.
use uuid::Uuid;

use crate::db::driver::DbEvent;
use crate::db::factory::DbFactory;

use super::DbWorker;

impl DbWorker {
    pub(super) async fn handle_connect(
        &mut self,
        config: crate::db::connection::ConnectionConfig,
    ) {
        let conn_id = config.id;
        let mut driver = match DbFactory::create(&config) {
            Ok(d) => d,
            Err(e) => {
                self.send_error(conn_id, None, e.to_string()).await;
                return;
            }
        };
        if let Err(e) = driver.connect(&config).await {
            self.send_error(conn_id, None, e.to_string()).await;
            return;
        }
        let db_result = driver.list_databases().await;

        // Query server version (best-effort — ignore errors)
        let server_version = match driver.execute("SELECT version()").await {
            Ok(qr) => qr
                .rows
                .first()
                .and_then(|row| row.first())
                .and_then(|v| match v {
                    crate::db::values::DbValue::Text(s) => Some(s.clone()),
                    _ => None,
                }),
            Err(_) => None,
        };

        self.connections.insert(conn_id, driver);
        match db_result {
            Ok(databases) => {
                let _ = self
                    .event_tx
                    .send(DbEvent::Connected {
                        conn_id,
                        databases,
                        server_version,
                    })
                    .await;
            }
            Err(e) => {
                self.send_error(
                    conn_id,
                    None,
                    format!("Connected but listing databases failed: {}", e),
                )
                .await;
            }
        }
    }

    pub(super) async fn handle_disconnect(&mut self, conn_id: Uuid) {
        if let Some(mut driver) = self.connections.remove(&conn_id) {
            let _ = driver.disconnect().await;
        }
        let _ = self
            .event_tx
            .send(DbEvent::Disconnected { conn_id })
            .await;
    }
}
