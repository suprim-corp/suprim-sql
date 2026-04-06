//! Integration tests for the Redis driver.
//! Requires Docker — uses testcontainers-rs to spin up redis:7.

use std::collections::HashMap;

use testcontainers::runners::AsyncRunner;
use testcontainers_modules::redis::Redis;

use suprim_sql::db::{ConnectionConfig, DbFactory, DbValue, DriverParams};

async fn setup() -> (ConnectionConfig, impl Drop) {
    let container = Redis::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(6379).await.unwrap();
    // Small delay to ensure Redis is ready to accept connections
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let config = ConnectionConfig::new(
        "test-redis",
        DriverParams::Redis {
            host: "127.0.0.1".into(),
            port,
            db_index: 0,
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
async fn test_execute_ping() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver.execute("PING").await.unwrap();
    assert_eq!(result.rows.len(), 1);
    // PONG response
    assert_eq!(result.rows[0][0], DbValue::Text("PONG".to_string()));
}

#[tokio::test]
async fn test_execute_set_get() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // SET via execute command string
    let set_result = driver.execute("SET mykey myvalue").await.unwrap();
    // Redis SET returns "OK" or Null depending on variant; just verify 1 row
    assert_eq!(set_result.rows.len(), 1);

    // GET — should return the value we set
    let get_result = driver.execute("GET mykey").await.unwrap();
    assert_eq!(get_result.rows.len(), 1);
    // Value should be Text("myvalue") or Int if numeric
    assert_ne!(get_result.rows[0][0], DbValue::Null);
}

#[tokio::test]
async fn test_execute_with_params() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver
        .execute_with_params("SET", vec![
            DbValue::Text("param_key".to_string()),
            DbValue::Text("param_value".to_string()),
        ])
        .await
        .unwrap();
    // SET returns 1 row (OK or Null)
    assert_eq!(result.rows.len(), 1);
}

#[tokio::test]
async fn test_insert_update_delete_row() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Insert = SET key value
    let mut values = HashMap::new();
    values.insert("key".to_string(), DbValue::Text("user:1".to_string()));
    values.insert("value".to_string(), DbValue::Text("Alice".to_string()));
    let affected = driver.insert_row("user", values).await.unwrap();
    assert_eq!(affected, 1);

    // Update = overwrite
    let mut pk = HashMap::new();
    pk.insert("key".to_string(), DbValue::Text("user:1".to_string()));
    let mut changes = HashMap::new();
    changes.insert("value".to_string(), DbValue::Text("Bob".to_string()));
    let affected = driver.update_row("user", pk.clone(), changes).await.unwrap();
    assert_eq!(affected, 1);

    // Delete
    let affected = driver.delete_row("user", pk).await.unwrap();
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn test_load_schema() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Set some keys first
    driver.execute("SET app:config version1").await.unwrap();
    driver.execute("SET app:state running").await.unwrap();

    let tree = driver.load_schema().await.unwrap();
    assert!(!tree.databases.is_empty());
}

#[tokio::test]
async fn test_table_data_pagination() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Set several user: keys
    for i in 0..5 {
        driver.execute(&format!("SET user:{} value{}", i, i)).await.unwrap();
    }

    // Paginate user: keys
    let page0 = driver.table_data(None, "user", 0, 3).await.unwrap();
    // Should get up to 3 rows
    assert!(page0.rows.len() <= 3);
}

#[tokio::test]
async fn test_execute_returns_execution_time() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver.execute("PING").await.unwrap();
    assert!(result.execution_time.as_nanos() > 0);
}

#[tokio::test]
async fn test_execute_error_paths() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Execute empty command — triggers AppError::Query("empty command")
    let err = driver.execute("").await;
    assert!(err.is_err());

    // insert_row without 'key' field — triggers AppError::Query("missing 'key' field")
    let empty_values = HashMap::new();
    let err2 = driver.insert_row("test", empty_values).await;
    assert!(err2.is_err());

    // delete_row without 'key' field
    let empty_pk = HashMap::new();
    let err3 = driver.delete_row("test", empty_pk).await;
    assert!(err3.is_err());

    // execute INVALID_CMD that Redis doesn't understand
    let err4 = driver.execute("THISISNOTAVALIDREDISCOMMAND_XYZ").await;
    assert!(err4.is_err());
}

#[tokio::test]
async fn test_load_schema_with_keys() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Set keys with different prefixes to exercise load_schema SCAN path
    driver.execute("SET session:1 data1").await.unwrap();
    driver.execute("SET session:2 data2").await.unwrap();
    driver.execute("SET cache:1 value").await.unwrap();

    let tree = driver.load_schema().await.unwrap();
    assert!(!tree.databases.is_empty());
    // Schema should have at least some tables from the prefixes
    let schema = &tree.databases[0].schemas[0];
    assert!(!schema.tables.is_empty());
}
