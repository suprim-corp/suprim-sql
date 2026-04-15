use super::*;
use crate::db::commands::{DbCommand, DbEvent};
use crate::premium::FreeTierGate;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

    fn test_gate() -> Arc<dyn crate::premium::PremiumGate> {
        Arc::new(FreeTierGate)
    }

    // ── Channel construction ──────────────────────────────────────────────────

    #[test]
    fn new_returns_channels_and_worker() {
        let (cmd_tx, _event_rx, _worker) = DbWorker::new(32, 32, test_gate());
        let _ = cmd_tx;
    }

    // ── Shutdown stops the run loop ───────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_stops_worker() {
        let (cmd_tx, _event_rx) = DbWorker::spawn(8, 8, test_gate());
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
        drop(cmd_tx);
    }

    // ── Execute on unknown conn_id → Error event ──────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn execute_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8, test_gate());

        let conn_id = Uuid::new_v4();
        let tab_id = Uuid::new_v4();
        cmd_tx
            .send(DbCommand::Execute {
                conn_id,
                tab_id,
                sql: "SELECT 1".to_string(),
                database: None,
            })
            .await
            .unwrap();

        let event = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }

    // ── ListDatabases on unknown conn → Error ────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn list_databases_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8, test_gate());
        let conn_id = Uuid::new_v4();
        cmd_tx
            .send(DbCommand::ListDatabases { conn_id })
            .await
            .unwrap();

        let event = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }

    // ── Insert/Update/Delete on unknown conn → Error ──────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn insert_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8, test_gate());
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

        let event = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn update_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8, test_gate());
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

        let event = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn delete_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8, test_gate());
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

        let event = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn load_table_data_unknown_conn_returns_error() {
        let (cmd_tx, mut event_rx) = DbWorker::spawn(8, 8, test_gate());
        let conn_id = Uuid::new_v4();
        let tab_id = Uuid::new_v4();
        cmd_tx
            .send(DbCommand::LoadTableData {
                conn_id,
                tab_id,
                database: None,
                schema: None,
                table: "users".to_string(),
                page: 0,
                page_size: 50,
                where_clause: None,
                order_clause: None,
            })
            .await
            .unwrap();

        let event = tokio::time::timeout(std::time::Duration::from_secs(2), event_rx.recv())
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(event, DbEvent::Error { .. }));
        cmd_tx.send(DbCommand::Shutdown).await.unwrap();
    }
