/// Row mutation handlers: insert, update, delete.
use uuid::Uuid;

use crate::db::driver::DbEvent;

use super::DbWorker;

impl DbWorker {
    pub(super) async fn handle_insert_row(
        &self,
        conn_id: Uuid,
        tab_id: Uuid,
        table: &str,
        values: std::collections::HashMap<String, crate::db::values::DbValue>,
    ) {
        if let Some(driver) = self.get_driver(conn_id, Some(tab_id)).await {
            match driver.insert_row(table, values).await {
                Ok(rows_affected) => {
                    let _ = self
                        .event_tx
                        .send(DbEvent::RowMutated {
                            tab_id,
                            rows_affected,
                        })
                        .await;
                }
                Err(e) => {
                    self.send_error(conn_id, Some(tab_id), e.to_string()).await;
                }
            }
        }
    }

    pub(super) async fn handle_update_row(
        &self,
        conn_id: Uuid,
        tab_id: Uuid,
        table: &str,
        pk: std::collections::HashMap<String, crate::db::values::DbValue>,
        changes: std::collections::HashMap<String, crate::db::values::DbValue>,
    ) {
        if let Some(driver) = self.get_driver(conn_id, Some(tab_id)).await {
            match driver.update_row(table, pk, changes).await {
                Ok(rows_affected) => {
                    let _ = self
                        .event_tx
                        .send(DbEvent::RowMutated {
                            tab_id,
                            rows_affected,
                        })
                        .await;
                }
                Err(e) => {
                    self.send_error(conn_id, Some(tab_id), e.to_string()).await;
                }
            }
        }
    }

    pub(super) async fn handle_delete_row(
        &self,
        conn_id: Uuid,
        tab_id: Uuid,
        table: &str,
        pk: std::collections::HashMap<String, crate::db::values::DbValue>,
    ) {
        if let Some(driver) = self.get_driver(conn_id, Some(tab_id)).await {
            match driver.delete_row(table, pk).await {
                Ok(rows_affected) => {
                    let _ = self
                        .event_tx
                        .send(DbEvent::RowMutated {
                            tab_id,
                            rows_affected,
                        })
                        .await;
                }
                Err(e) => {
                    self.send_error(conn_id, Some(tab_id), e.to_string()).await;
                }
            }
        }
    }
}
