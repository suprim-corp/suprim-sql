//! Integration tests for the SQLite driver.
//! Uses an in-memory database — no Docker required.

use std::collections::HashMap;

use suprim_core::db::{ConnectionConfig, DbFactory, DriverParams};

fn sqlite_memory_config() -> ConnectionConfig {
    ConnectionConfig::new(
        "test-sqlite",
        DriverParams::Sqlite {
            path: ":memory:".into(),
        },
    )
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_connect_ok() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();
    assert!(driver.is_connected());
    driver.ping().await.unwrap();
    driver.disconnect().await.unwrap();
    assert!(!driver.is_connected());
}

#[tokio::test]
async fn test_execute_select() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver.execute("SELECT 42 AS n, 'hello' AS s").await.unwrap();
    assert_eq!(result.columns.len(), 2);
    assert_eq!(result.rows.len(), 1);
    assert!(result.execution_time.as_nanos() > 0);
}

#[tokio::test]
async fn test_execute_with_params() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver
        .execute_with_params(
            "SELECT ? AS val",
            vec![suprim_core::db::DbValue::Text("world".to_string())],
        )
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);
}

#[tokio::test]
async fn test_execute_insert_update_delete() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL)")
        .await
        .unwrap();

    // Insert
    let mut values = HashMap::new();
    values.insert("id".to_string(), suprim_core::db::DbValue::Int(1));
    values.insert("name".to_string(), suprim_core::db::DbValue::Text("Alice".to_string()));
    let affected = driver.insert_row("users", values).await.unwrap();
    assert_eq!(affected, 1);

    // Update
    let mut pk = HashMap::new();
    pk.insert("id".to_string(), suprim_core::db::DbValue::Int(1));
    let mut changes = HashMap::new();
    changes.insert("name".to_string(), suprim_core::db::DbValue::Text("Bob".to_string()));
    let affected = driver.update_row("users", pk.clone(), changes).await.unwrap();
    assert_eq!(affected, 1);

    // Verify update
    let result = driver.execute("SELECT name FROM users WHERE id=1").await.unwrap();
    assert_eq!(result.rows[0][0], suprim_core::db::DbValue::Text("Bob".to_string()));

    // Delete
    let affected = driver.delete_row("users", pk).await.unwrap();
    assert_eq!(affected, 1);
}

#[tokio::test]
async fn test_load_schema() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute(
            "CREATE TABLE products (\
                id INTEGER PRIMARY KEY,\
                name TEXT NOT NULL,\
                price REAL\
             )",
        )
        .await
        .unwrap();

    let tree = driver.load_schema().await.unwrap();
    assert_eq!(tree.databases.len(), 1);
    let schema = &tree.databases[0].schemas[0];
    let table = schema.tables.iter().find(|t| t.name == "products").unwrap();
    assert_eq!(table.columns.len(), 3);
    let id_col = table.columns.iter().find(|c| c.name == "id").unwrap();
    assert!(id_col.is_primary_key);
}

#[tokio::test]
async fn test_table_data_pagination() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute("CREATE TABLE pag (id INTEGER PRIMARY KEY)")
        .await
        .unwrap();

    for i in 1..=15i64 {
        driver
            .execute_with_params(
                "INSERT INTO pag(id) VALUES(?)",
                vec![suprim_core::db::DbValue::Int(i)],
            )
            .await
            .unwrap();
    }

    let page0 = driver.table_data(None, "pag", 0, 10).await.unwrap();
    assert_eq!(page0.rows.len(), 10);

    let page1 = driver.table_data(None, "pag", 1, 10).await.unwrap();
    assert_eq!(page1.rows.len(), 5);

    let page2 = driver.table_data(None, "pag", 2, 10).await.unwrap();
    assert_eq!(page2.rows.len(), 0);
}

#[tokio::test]
async fn test_load_schema_with_index() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute("CREATE TABLE idx_t (id INTEGER PRIMARY KEY, email TEXT)")
        .await
        .unwrap();
    driver
        .execute("CREATE UNIQUE INDEX uidx_email ON idx_t(email)")
        .await
        .unwrap();

    let tree = driver.load_schema().await.unwrap();
    let schema = &tree.databases[0].schemas[0];
    let table = schema.tables.iter().find(|t| t.name == "idx_t").unwrap();
    let uidx = table.indexes.iter().find(|i| i.name == "uidx_email").unwrap();
    assert!(uidx.is_unique);
}

#[tokio::test]
async fn test_load_schema_with_foreign_key() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver.execute("PRAGMA foreign_keys = ON").await.unwrap();
    driver
        .execute("CREATE TABLE parent (id INTEGER PRIMARY KEY)")
        .await
        .unwrap();
    driver
        .execute(
            "CREATE TABLE child (\
                id INTEGER PRIMARY KEY,\
                parent_id INTEGER REFERENCES parent(id)\
             )",
        )
        .await
        .unwrap();

    let tree = driver.load_schema().await.unwrap();
    let schema = &tree.databases[0].schemas[0];
    let child = schema.tables.iter().find(|t| t.name == "child").unwrap();
    assert!(!child.foreign_keys.is_empty());
    assert_eq!(child.foreign_keys[0].ref_table, "parent");
}

#[tokio::test]
async fn test_execute_returns_execution_time() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver.execute("SELECT 1").await.unwrap();
    assert!(result.execution_time.as_nanos() > 0);
}

#[tokio::test]
async fn test_all_db_value_types_roundtrip() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute(
            "CREATE TABLE types_test (\
                id INTEGER PRIMARY KEY,\
                name TEXT,\
                score REAL,\
                active INTEGER\
             )",
        )
        .await
        .unwrap();

    // Insert via execute
    driver
        .execute("INSERT INTO types_test VALUES (1, 'Alice', 9.5, 1)")
        .await
        .unwrap();

    let result = driver
        .execute("SELECT * FROM types_test WHERE id = 1")
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.columns.len(), 4);
    // id = 1 (Int), name = 'Alice' (Text), score = 9.5 (Float), active = 1 (Int)
    let row = &result.rows[0];
    assert_eq!(row[0], suprim_core::db::DbValue::Int(1));
    assert_eq!(row[1], suprim_core::db::DbValue::Text("Alice".to_string()));
    // score may be Float or Text depending on type affinity
    assert!(
        matches!(&row[2], suprim_core::db::DbValue::Float(f) if *f == 9.5)
            || matches!(&row[2], suprim_core::db::DbValue::Text(s) if s == "9.5")
    );
}

#[tokio::test]
async fn test_execute_error_paths() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Execute invalid SQL — triggers .map_err in execute()
    let err = driver.execute("INVALID SQL STATEMENT !!!").await;
    assert!(err.is_err());

    // Execute with params on bad SQL
    let err2 = driver
        .execute_with_params("SELECT ? FROM nonexistent_xyz", vec![suprim_core::db::DbValue::Int(1)])
        .await;
    assert!(err2.is_err());

    // table_data on nonexistent table — triggers .map_err in table_data()
    let err3 = driver.table_data(None, "nonexistent_xyz", 0, 10).await;
    assert!(err3.is_err());

    // insert_row on nonexistent table
    let mut vals = HashMap::new();
    vals.insert("id".to_string(), suprim_core::db::DbValue::Int(1));
    let err4 = driver.insert_row("nonexistent_xyz", vals).await;
    assert!(err4.is_err());

    // update_row on nonexistent table
    let mut pk = HashMap::new();
    pk.insert("id".to_string(), suprim_core::db::DbValue::Int(1));
    let mut changes = HashMap::new();
    changes.insert("v".to_string(), suprim_core::db::DbValue::Int(2));
    let err5 = driver.update_row("nonexistent_xyz", pk.clone(), changes).await;
    assert!(err5.is_err());

    // delete_row on nonexistent table
    let err6 = driver.delete_row("nonexistent_xyz", pk).await;
    assert!(err6.is_err());
}

#[tokio::test]
async fn test_bind_all_db_value_types() {
    let config = sqlite_memory_config();
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute(
            "CREATE TABLE bind_test (\
                id INT, null_col TEXT, bool_col INT, \
                float_col REAL, bytes_col BLOB, json_col TEXT, ts_col TEXT\
             )",
        )
        .await
        .unwrap();

    // Bind all DbValue types to exercise bind_db_value() lines 517-524
    let ts = chrono::DateTime::<chrono::Utc>::from_timestamp(1_000_000, 0).unwrap();
    driver
        .execute_with_params(
            "INSERT INTO bind_test VALUES(?, ?, ?, ?, ?, ?, ?)",
            vec![
                suprim_core::db::DbValue::Int(1),
                suprim_core::db::DbValue::Null,
                suprim_core::db::DbValue::Bool(true),
                suprim_core::db::DbValue::Float(3.14),
                suprim_core::db::DbValue::Bytes(vec![1, 2, 3]),
                suprim_core::db::DbValue::Json(serde_json::json!({"x": 1})),
                suprim_core::db::DbValue::Timestamp(ts),
            ],
        )
        .await
        .unwrap();

    let result = driver
        .execute("SELECT * FROM bind_test WHERE id = 1")
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);
}
