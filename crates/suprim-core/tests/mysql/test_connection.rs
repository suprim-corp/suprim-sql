use crate::helpers;
use suprim_core::db::connection::DriverParams;
use suprim_core::db::driver::DatabaseDriver;
use suprim_core::db::drivers::mysql::MysqlDriver;

#[tokio::test]
async fn connect_and_ping() {
    let mut driver = MysqlDriver::new();
    assert!(!driver.is_connected());

    driver.connect(&helpers::test_config("testdb")).await.unwrap();
    assert!(driver.is_connected());
    driver.ping().await.unwrap();
    driver.disconnect().await.unwrap();
    assert!(!driver.is_connected());
}

#[tokio::test]
async fn execute_raw_sql() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT 1 + 1 AS sum").await.unwrap();
    assert_eq!(result.columns.len(), 1);
    assert_eq!(result.columns[0].name, "sum");
    assert_eq!(result.rows.len(), 1);
}

#[tokio::test]
async fn connect_wrong_params_returns_error() {
    let mut driver = MysqlDriver::new();
    let config = suprim_core::db::connection::ConnectionConfig::new(
        "bad",
        DriverParams::Postgres {
            host: "localhost".into(),
            port: 5432,
            database: "db".into(),
            user: "user".into(),
            password_key: "key".into(),
        },
    );
    let err = driver.connect(&config).await;
    assert!(err.is_err());
}
