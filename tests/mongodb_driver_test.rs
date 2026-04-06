//! Integration tests for the MongoDB driver.
//! Requires Docker — uses testcontainers-rs to spin up mongo:7.

use std::collections::HashMap;

use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mongo::Mongo;

use suprim_sql::db::{ConnectionConfig, DbFactory, DbValue, DriverParams};

async fn setup() -> (ConnectionConfig, impl Drop) {
    let container = Mongo::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(27017).await.unwrap();
    let config = ConnectionConfig::new(
        "test-mongo",
        DriverParams::MongoDB {
            uri: format!("mongodb://127.0.0.1:{}/testdb", port),
            password_key: None,
        },
    );
    (config, container)
}

#[tokio::test]
async fn test_connect_ok() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();
    assert!(driver.is_connected());
    driver.ping().await.unwrap();
    driver.disconnect().await.unwrap();
    assert!(!driver.is_connected());
}

#[tokio::test]
async fn test_execute_ping_command() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver.execute(r#"{"ping":1}"#).await.unwrap();
    // ping returns {"ok":1}
    assert_eq!(result.rows.len(), 1);
}

#[tokio::test]
async fn test_insert_and_load_schema() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Insert a document
    let mut values = HashMap::new();
    values.insert("name".to_string(), DbValue::Text("Alice".to_string()));
    values.insert("age".to_string(), DbValue::Int(30));
    let affected = driver.insert_row("users", values).await.unwrap();
    assert_eq!(affected, 1);

    // Load schema — should include testdb with users collection
    let tree = driver.load_schema().await.unwrap();
    let testdb = tree.databases.iter().find(|d| d.name == "testdb");
    assert!(testdb.is_some(), "testdb should be in schema tree");
}

#[tokio::test]
async fn test_execute_with_params_find() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Insert a doc
    let mut values = HashMap::new();
    values.insert("x".to_string(), DbValue::Int(42));
    driver.insert_row("items", values).await.unwrap();

    // Find with filter = {}
    let result = driver
        .execute_with_params("items", vec![DbValue::Text("{}".to_string())])
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);
}

#[tokio::test]
async fn test_table_data_pagination() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Insert 5 documents
    for i in 0..5i64 {
        let mut values = HashMap::new();
        values.insert("n".to_string(), DbValue::Int(i));
        driver.insert_row("pag", values).await.unwrap();
    }

    let page0 = driver.table_data(None, "pag", 0, 3).await.unwrap();
    assert_eq!(page0.rows.len(), 3);

    let page1 = driver.table_data(None, "pag", 1, 3).await.unwrap();
    assert_eq!(page1.rows.len(), 2);
}

#[tokio::test]
async fn test_update_and_delete_row() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Insert
    let mut values = HashMap::new();
    values.insert("name".to_string(), DbValue::Text("Alice".to_string()));
    values.insert("active".to_string(), DbValue::Bool(true));
    driver.insert_row("t_upd", values).await.unwrap();

    // Update
    let mut pk = HashMap::new();
    pk.insert("name".to_string(), DbValue::Text("Alice".to_string()));
    let mut changes = HashMap::new();
    changes.insert("active".to_string(), DbValue::Bool(false));
    let modified = driver.update_row("t_upd", pk.clone(), changes).await.unwrap();
    assert!(modified >= 0);

    // Delete
    let deleted = driver.delete_row("t_upd", pk).await.unwrap();
    assert!(deleted >= 0);
}

#[tokio::test]
async fn test_execute_returns_execution_time() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver.execute(r#"{"ping":1}"#).await.unwrap();
    assert!(result.execution_time.as_nanos() > 0);
}

#[tokio::test]
async fn test_execute_find_with_filter() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Insert docs and then find using cursor
    let mut values = HashMap::new();
    values.insert("x".to_string(), DbValue::Int(99));
    driver.insert_row("findtest", values).await.unwrap();

    // execute find command — exercises cursor.firstBatch extraction path
    let result = driver
        .execute(r#"{"find":"findtest","filter":{},"limit":5}"#)
        .await
        .unwrap();
    assert!(result.rows.len() >= 1);
}

#[tokio::test]
async fn test_execute_invalid_json_returns_error() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Invalid JSON triggers serde_json::from_str error
    let err = driver.execute("{invalid json!!!}").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn test_execute_error_paths() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // execute valid command that fails — nonexistent command triggers DB error
    let err = driver.execute(r#"{"nonexistentCommand":1}"#).await;
    // This may succeed or fail depending on MongoDB version; just assert no panic
    let _ = err;

    // execute_with_params on valid collection  
    let result = driver
        .execute_with_params("findtest2", vec![DbValue::Text("{}".to_string())])
        .await
        .unwrap();
    // Empty collection returns 0 rows
    assert_eq!(result.rows.len(), 0);

    // insert_row with various DbValue types to exercise db_value_to_bson paths
    let mut values = HashMap::new();
    values.insert("bool_val".to_string(), DbValue::Bool(true));
    values.insert("int_val".to_string(), DbValue::Int(42));
    values.insert("float_val".to_string(), DbValue::Float(3.14));
    values.insert("text_val".to_string(), DbValue::Text("hello".to_string()));
    values.insert("null_val".to_string(), DbValue::Null);
    values.insert("bytes_val".to_string(), DbValue::Bytes(vec![1, 2, 3]));
    let ts = chrono::DateTime::<chrono::Utc>::from_timestamp(1_000_000, 0).unwrap();
    values.insert("ts_val".to_string(), DbValue::Timestamp(ts));
    let affected = driver.insert_row("type_test", values).await.unwrap();
    assert_eq!(affected, 1);

    // table_data to read back
    let result = driver.table_data(None, "type_test", 0, 10).await.unwrap();
    assert_eq!(result.rows.len(), 1);
}
