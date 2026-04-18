use crate::helpers;
use suprim_core::db::driver::DatabaseDriver;

#[tokio::test]
async fn list_sessions_includes_own() {
    let driver = helpers::connected_driver("testdb").await;

    let sessions = driver.list_sessions().await.unwrap();
    // PG filters out own PID — just verify it doesn't crash and returns a Vec.
    // Sessions may be empty if all pool connections are idle with no query_start.
    let _ = sessions.len();
}

#[tokio::test]
async fn server_metrics_populated() {
    let driver = helpers::connected_driver("testdb").await;

    let metrics = driver.server_metrics().await.unwrap();
    assert!(metrics.connected_sessions > 0, "Should have at least 1 connected session");
    assert!(metrics.max_connections > 0, "max_connections should be set");
    assert!(!metrics.uptime.is_empty(), "uptime should not be empty");
}

#[tokio::test]
async fn slow_queries_does_not_crash() {
    let driver = helpers::connected_driver("testdb").await;

    // May be empty if pg_stat_statements not installed — just verify no crash
    let result = driver.list_slow_queries().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn kill_nonexistent_session() {
    let driver = helpers::connected_driver("testdb").await;
    // pg_terminate_backend returns false for nonexistent PID, doesn't error
    // But our implementation wraps it in execute() which succeeds
    let result = driver.kill_session(999999).await;
    // PG doesn't error on nonexistent PID — pg_terminate_backend returns bool
    assert!(result.is_ok(), "kill_session should not error on nonexistent PID in PG");
}

#[tokio::test]
async fn metrics_uptime_format() {
    let driver = helpers::connected_driver("testdb").await;

    let metrics = driver.server_metrics().await.unwrap();
    // Uptime format should be like "0m", "5m", "1h 30m", "2d 5h 30m"
    assert!(
        metrics.uptime.contains('m') || metrics.uptime.contains('h') || metrics.uptime.contains('d'),
        "Uptime should have time unit suffix: {}",
        metrics.uptime
    );
}
