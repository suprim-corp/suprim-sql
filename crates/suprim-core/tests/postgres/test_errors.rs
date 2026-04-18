use crate::helpers;
use std::collections::HashMap;
use suprim_core::db::driver::DatabaseDriver;
use suprim_core::db::drivers::postgres::PostgresDriver;
use suprim_core::db::values::DbValue;

// ── Connection errors ────────────────────────────────────────────────────────

#[tokio::test]
async fn connect_wrong_host() {
    let mut driver = PostgresDriver::new();
    let config = suprim_core::db::connection::ConnectionConfig::new(
        "bad",
        suprim_core::db::connection::DriverParams::Postgres {
            host: "192.0.2.1".into(), // RFC 5737 TEST-NET — guaranteed unreachable
            port: 5432,
            database: "db".into(),
            user: "postgres".into(),
            password_key: "pass".into(),
        },
    );
    let err = driver.connect(&config).await;
    assert!(err.is_err(), "Should fail on unreachable host");
}

#[tokio::test]
async fn connect_wrong_credentials() {
    let port: u16 = std::env::var("PG_TEST_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5433);
    let mut driver = PostgresDriver::new();
    let config = suprim_core::db::connection::ConnectionConfig::new(
        "bad",
        suprim_core::db::connection::DriverParams::Postgres {
            host: "127.0.0.1".into(),
            port,
            database: "testdb".into(),
            user: "postgres".into(),
            password_key: "WRONG_PASSWORD".into(),
        },
    );
    let err = driver.connect(&config).await;
    assert!(err.is_err(), "Should fail with wrong password");
}

#[tokio::test]
async fn connect_wrong_database() {
    let port: u16 = std::env::var("PG_TEST_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5433);
    let mut driver = PostgresDriver::new();
    let config = suprim_core::db::connection::ConnectionConfig::new(
        "bad",
        suprim_core::db::connection::DriverParams::Postgres {
            host: "127.0.0.1".into(),
            port,
            database: "nonexistent_db_xyz".into(),
            user: "postgres".into(),
            password_key: "testpass".into(),
        },
    );
    let err = driver.connect(&config).await;
    assert!(err.is_err(), "Should fail on nonexistent database");
}

// ── Query errors ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn execute_invalid_sql() {
    let driver = helpers::connected_driver("testdb").await;
    let err = driver.execute("SELECT * FROM THIS IS NOT VALID SQL").await;
    assert!(err.is_err(), "Should fail on invalid SQL");
}

#[tokio::test]
async fn execute_not_connected() {
    let driver = PostgresDriver::new();
    let err = driver.execute("SELECT 1").await;
    assert!(err.is_err(), "Should fail when not connected");
}

#[tokio::test]
async fn ping_not_connected() {
    let driver = PostgresDriver::new();
    let err = driver.ping().await;
    assert!(err.is_err(), "Should fail when not connected");
}

// ── table_data errors ────────────────────────────────────────────────────────

#[tokio::test]
async fn table_data_nonexistent_table() {
    let driver = helpers::connected_driver("testdb").await;
    let err = driver
        .table_data(Some("testdb"), Some("public"), "nonexistent_table_xyz", 0, 50, None, None)
        .await;
    assert!(err.is_err(), "Should fail on nonexistent table");
}

#[tokio::test]
async fn table_data_invalid_where() {
    let driver = helpers::connected_driver("testdb").await;
    let err = driver
        .table_data(
            Some("testdb"), Some("public"), "users", 0, 50,
            Some("INVALID CLAUSE %%%"), None,
        )
        .await;
    assert!(err.is_err(), "Should fail on invalid WHERE clause");
}

#[tokio::test]
async fn table_data_invalid_order() {
    let driver = helpers::connected_driver("testdb").await;
    let err = driver
        .table_data(
            Some("testdb"), Some("public"), "users", 0, 50,
            None, Some("nonexistent_column DESC"),
        )
        .await;
    assert!(err.is_err(), "Should fail on nonexistent ORDER BY column");
}

// ── CRUD errors ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn insert_duplicate_unique() {
    let driver = helpers::connected_driver("testdb").await;

    // First ensure alice@test.com exists
    let check = driver
        .table_data(Some("testdb"), Some("public"), "users", 0, 1, Some("email = 'alice@test.com'"), None)
        .await
        .unwrap();
    assert_eq!(check.rows.len(), 1, "alice@test.com should exist");

    let mut values = HashMap::new();
    values.insert("name".to_string(), DbValue::Text("Dup".to_string()));
    values.insert("email".to_string(), DbValue::Text("alice@test.com".to_string()));

    let err = driver.insert_row("users", values).await;
    assert!(err.is_err(), "Should fail on duplicate unique key");
}

#[tokio::test]
async fn insert_violate_fk() {
    let driver = helpers::connected_driver("testdb").await;
    let mut values = HashMap::new();
    values.insert("user_id".to_string(), DbValue::Int(99999)); // nonexistent user
    values.insert("total".to_string(), DbValue::Float(10.0));
    values.insert("status".to_string(), DbValue::Text("pending".to_string()));

    let err = driver.insert_row("orders", values).await;
    assert!(err.is_err(), "Should fail on FK violation");
}

#[tokio::test]
async fn insert_not_null_violation() {
    let driver = helpers::connected_driver("testdb").await;
    let mut values = HashMap::new();
    // name is NOT NULL but we only insert email
    values.insert("email".to_string(), DbValue::Text("noname@test.com".to_string()));

    let err = driver.insert_row("users", values).await;
    assert!(err.is_err(), "Should fail when NOT NULL column is missing");
}

#[tokio::test]
async fn update_nonexistent_pk_returns_zero() {
    let driver = helpers::connected_driver("testdb").await;
    let mut pk = HashMap::new();
    pk.insert("id".to_string(), DbValue::Int(999999));
    let mut changes = HashMap::new();
    changes.insert("name".to_string(), DbValue::Text("ghost".to_string()));

    let affected = driver.update_row("users", pk, changes).await.unwrap();
    assert_eq!(affected, 0, "Should affect 0 rows when PK doesn't exist");
}

#[tokio::test]
async fn delete_nonexistent_pk_returns_zero() {
    let driver = helpers::connected_driver("testdb").await;
    let mut pk = HashMap::new();
    pk.insert("id".to_string(), DbValue::Int(999999));

    let affected = driver.delete_row("users", pk).await.unwrap();
    assert_eq!(affected, 0, "Should affect 0 rows when PK doesn't exist");
}

// ── DDL errors ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn drop_nonexistent_table() {
    let driver = helpers::connected_driver("testdb").await;
    let err = driver.drop_table("public", "nonexistent_table_xyz").await;
    assert!(err.is_err(), "Should fail on nonexistent table");
}

#[tokio::test]
async fn truncate_nonexistent_table() {
    let driver = helpers::connected_driver("testdb").await;
    let err = driver.truncate_table("public", "nonexistent_table_xyz").await;
    assert!(err.is_err(), "Should fail on nonexistent table");
}

#[tokio::test]
async fn create_duplicate_database() {
    let driver = helpers::connected_driver("testdb").await;
    // testdb already exists
    let err = driver.create_database("testdb").await;
    assert!(err.is_err(), "Should fail when database already exists");
}

// ── Cross-database errors ────────────────────────────────────────────────────

#[tokio::test]
async fn execute_on_nonexistent_database() {
    let driver = helpers::connected_driver("testdb").await;
    let err = driver.execute_on_database("SELECT 1", "nonexistent_db_xyz").await;
    assert!(err.is_err(), "Should fail on nonexistent database");
}
