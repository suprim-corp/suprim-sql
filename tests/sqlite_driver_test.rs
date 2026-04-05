//! Integration tests for the SQLite driver.
//! Uses an in-memory database — no Docker required.

use suprim_sql::db::{ConnectionConfig, DriverParams};

fn sqlite_memory_config() -> ConnectionConfig {
    ConnectionConfig::new(
        "test-sqlite",
        DriverParams::Sqlite {
            path: ":memory:".into(),
        },
    )
}

#[tokio::test]
#[ignore = "driver not yet implemented"]
async fn test_connect_ok() {
    let _config = sqlite_memory_config();
    // TODO: DbFactory::create(&config).connect(&config).await.unwrap()
    todo!()
}

#[tokio::test]
#[ignore = "driver not yet implemented"]
async fn test_execute_select() {
    todo!()
}

#[tokio::test]
#[ignore = "driver not yet implemented"]
async fn test_execute_insert_update_delete() {
    todo!()
}

#[tokio::test]
#[ignore = "driver not yet implemented"]
async fn test_load_schema() {
    todo!()
}

#[tokio::test]
#[ignore = "driver not yet implemented"]
async fn test_table_data_pagination() {
    todo!()
}

#[tokio::test]
#[ignore = "driver not yet implemented"]
async fn test_transaction_rollback() {
    todo!()
}
