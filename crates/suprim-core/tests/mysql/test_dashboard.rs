use crate::helpers;
use suprim_core::db::driver::DatabaseDriver;

#[tokio::test]
async fn list_sessions_includes_own() {
    let driver = helpers::connected_driver("testdb").await;

    let sessions = driver.list_sessions().await.unwrap();
    assert!(!sessions.is_empty(), "Should have at least our own session");
    assert!(sessions.iter().any(|s| s.database == "testdb"),
        "Should find a session connected to testdb");
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

    // May be empty if performance_schema not populated — just verify no crash
    let result = driver.list_slow_queries().await;
    assert!(result.is_ok());
}
