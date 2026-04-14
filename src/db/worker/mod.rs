/// Asynchronous DB worker — owns all connections, processes commands from the
/// UI thread via mpsc channels. Command dispatch is in `command_handlers.rs`.
mod command_handlers;
mod handle_connection;
mod handle_mutation;
mod handle_query;
#[cfg(test)]
mod tests;

use std::collections::HashMap;

use tokio::sync::mpsc;
use uuid::Uuid;

use crate::db::driver::{DbCommand, DbEvent, DatabaseDriver};
use crate::db::ssh_tunnel::SshTunnel;

/// Asynchronous worker that owns all DB connections and processes commands
/// from the UI thread via a channel.
///
/// Run this on a dedicated `tokio::spawn` task. The UI sends `DbCommand`
/// values via the `cmd_tx` sender it receives from `DbWorker::spawn()`.
pub struct DbWorker {
    cmd_rx: mpsc::Receiver<DbCommand>,
    pub(crate) event_tx: mpsc::Sender<DbEvent>,
    /// Active connections keyed by conn_id
    pub(crate) connections: HashMap<Uuid, Box<dyn DatabaseDriver>>,
    /// Active SSH tunnels keyed by conn_id (dropped on disconnect)
    pub(crate) tunnels: HashMap<Uuid, SshTunnel>,
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
                tunnels: HashMap::new(),
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

    // ── Helpers (used by command_handlers) ────────────────────────────────────

    /// Send an error event back to the UI.
    pub(crate) async fn send_error(&self, conn_id: Uuid, tab_id: Option<Uuid>, message: String) {
        let _ = self
            .event_tx
            .send(DbEvent::Error {
                conn_id: Some(conn_id),
                tab_id,
                message,
            })
            .await;
    }

    /// Look up a connection by ID, sending an error event if not found.
    pub(crate) async fn get_driver(
        &self,
        conn_id: Uuid,
        tab_id: Option<Uuid>,
    ) -> Option<&dyn DatabaseDriver> {
        match self.connections.get(&conn_id) {
            Some(driver) => Some(driver.as_ref()),
            None => {
                self.send_error(conn_id, tab_id, "Not connected".to_string())
                    .await;
                None
            }
        }
    }

    /// Helper for DDL commands — runs the operation, sends DdlCompleted or Error.
    pub(crate) async fn handle_ddl<F>(
        &self,
        conn_id: Uuid,
        database: &str,
        schema_name: &str,
        op: F,
    ) where
        F: std::future::Future<Output = crate::error::Result<()>>,
    {
        match op.await {
            Ok(()) => {
                let _ = self
                    .event_tx
                    .send(DbEvent::DdlCompleted {
                        conn_id,
                        database: database.to_string(),
                        schema_name: schema_name.to_string(),
                    })
                    .await;
            }
            Err(e) => {
                self.send_error(conn_id, None, e.to_string()).await;
            }
        }
    }
}
