//! Integration tests for the MSSQL driver.
//! Requires Docker — uses testcontainers-rs to spin up SQL Server.
//! Note: SQL Server Docker image is x86_64 only.
//! On Apple Silicon, Docker Desktop Rosetta emulation must be enabled.

use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mssql_server::MssqlServer;
use testcontainers_modules::testcontainers::ImageExt;

use suprim_sql::db::connection::{ConnectionConfig, DriverParams};
use suprim_sql::db::driver::DatabaseDriver;
use suprim_sql::db::mssql::MssqlDriver;

// ─── Helpers ─────────────────────────────────────────────────────────────────

const SA_PASSWORD: &str = "yourStrong(!)Password";

async fn setup() -> (MssqlDriver, impl Drop) {
    let container = MssqlServer::default()
        .with_accept_eula()
        .with_platform("linux/amd64")
        .start()
        .await
        .unwrap();
    let port = container.get_host_port_ipv4(1433).await.unwrap();

    let config = ConnectionConfig::new(
        "test-mssql",
        DriverParams::Mssql {
            host: "127.0.0.1".into(),
            port,
            database: "master".into(),
            user: "sa".into(),
            password_key: SA_PASSWORD.into(),
        },
    );

    let mut driver = MssqlDriver::new();
    driver.connect(&config).await.unwrap();
    (driver, container)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_connect_ok() {
    let (driver, _container) = setup().await;
    assert!(driver.is_connected());
}

#[tokio::test]
async fn test_ping_ok() {
    let (driver, _container) = setup().await;
    driver.ping().await.unwrap();
}

#[tokio::test]
async fn test_disconnect_ok() {
    let (mut driver, _container) = setup().await;
    driver.disconnect().await.unwrap();
    assert!(!driver.is_connected());
}

#[tokio::test]
async fn test_execute_mut_select() {
    let (mut driver, _container) = setup().await;

    let result = driver.execute_mut("SELECT 1 AS n").await.unwrap();
    assert_eq!(result.columns.len(), 1);
    assert_eq!(result.rows.len(), 1);
    assert!(result.execution_time.as_nanos() > 0);
}

#[tokio::test]
async fn test_execute_mut_create_and_insert() {
    let (mut driver, _container) = setup().await;

    driver
        .execute_mut(
            "CREATE TABLE #tmp_test (\
                id INT PRIMARY KEY,\
                name NVARCHAR(100) NOT NULL\
             )",
        )
        .await
        .unwrap();

    driver
        .execute_mut("INSERT INTO #tmp_test VALUES (1, N'Alice'), (2, N'Bob')")
        .await
        .unwrap();

    let result = driver
        .execute_mut("SELECT * FROM #tmp_test ORDER BY id")
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 2);
}

#[tokio::test]
async fn test_execute_mut_type_mapping() {
    let (mut driver, _container) = setup().await;

    // Test various SQL Server types
    let result = driver
        .execute_mut(
            "SELECT \
                CAST(1 AS BIT) AS bit_col, \
                CAST(42 AS TINYINT) AS int1_col, \
                CAST(100 AS SMALLINT) AS int2_col, \
                CAST(1000 AS INT) AS int4_col, \
                CAST(100000 AS BIGINT) AS int8_col, \
                CAST(3.14 AS REAL) AS float4_col, \
                CAST(2.718 AS FLOAT) AS float8_col, \
                N'hello' AS text_col, \
                GETDATE() AS datetime_col",
        )
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.columns.len(), 9);
}

#[tokio::test]
async fn test_execute_mut_binary_type() {
    let (mut driver, _container) = setup().await;

    let result = driver
        .execute_mut("SELECT CAST(0x0102 AS VARBINARY(10)) AS bin_col")
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 1);
}

#[tokio::test]
async fn test_load_schema_mut() {
    let (mut driver, _container) = setup().await;

    // Create a table to verify it shows in schema
    driver
        .execute_mut(
            "IF OBJECT_ID('dbo.schema_test_t', 'U') IS NOT NULL DROP TABLE dbo.schema_test_t; \
             CREATE TABLE dbo.schema_test_t (id INT PRIMARY KEY, name NVARCHAR(50))",
        )
        .await
        .unwrap();

    let tree = driver.load_schema_mut().await.unwrap();
    assert!(!tree.databases.is_empty());

    // Find the table in the schema
    let db = &tree.databases[0];
    let schema = db.schemas.iter().find(|s| s.name == "dbo");
    assert!(schema.is_some(), "dbo schema should exist");

    if let Some(s) = schema {
        let table = s.tables.iter().find(|t| t.name == "schema_test_t");
        assert!(table.is_some(), "schema_test_t should be in schema");
    }
}

#[tokio::test]
async fn test_execute_returns_empty_for_ddl() {
    let (mut driver, _container) = setup().await;

    // DDL creates a temp table — fetch_first_result returns empty rows
    let result = driver
        .execute_mut("CREATE TABLE #ddl_only (id INT)")
        .await
        .unwrap();

    // DDL statement returns 0 rows
    assert_eq!(result.rows.len(), 0);
}

#[tokio::test]
async fn test_execute_wrong_params_when_not_connected() {
    let config = ConnectionConfig::new(
        "bad",
        DriverParams::Sqlite {
            path: "/tmp/test.db".into(),
        },
    );
    let mut driver = MssqlDriver::new();
    let err = driver.connect(&config).await.unwrap_err();
    assert!(matches!(
        err,
        suprim_sql::error::AppError::Connection(_)
    ));
}
