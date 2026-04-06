//! Integration tests for the MySQL driver.
//! Requires Docker — uses testcontainers-rs to spin up mysql:8.

use std::collections::HashMap;

use testcontainers::runners::AsyncRunner;
use testcontainers_modules::mysql::Mysql;

use suprim_sql::db::{ConnectionConfig, DbFactory, DbValue, DriverParams};

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn setup() -> (ConnectionConfig, impl Drop) {
    let container = Mysql::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(3306).await.unwrap();

    let config = ConnectionConfig::new(
        "test-mysql",
        DriverParams::Mysql {
            host: "127.0.0.1".into(),
            port,
            database: "mysql".into(),
            user: "root".into(),
            // testcontainers-modules mysql default root password is empty
            password_key: "".into(),
        },
    );
    (config, container)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

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
async fn test_connect_wrong_password() {
    let (config, _container) = setup().await;
    let bad_config = ConnectionConfig::new(
        "test-mysql-bad",
        DriverParams::Mysql {
            host: "127.0.0.1".into(),
            port: if let DriverParams::Mysql { port, .. } = &config.params { *port } else { 3306 },
            database: "mysql".into(),
            user: "root".into(),
            password_key: "wrong_password_12345".into(),
        },
    );
    let mut driver = DbFactory::create(&bad_config).unwrap();
    assert!(driver.connect(&bad_config).await.is_err());
}

#[tokio::test]
async fn test_execute_select() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver.execute("SELECT 1 AS n").await.unwrap();
    assert_eq!(result.columns.len(), 1);
    assert_eq!(result.rows.len(), 1);
    assert!(result.execution_time.as_nanos() > 0);
}

#[tokio::test]
async fn test_execute_with_params() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver
        .execute_with_params("SELECT ? AS val", vec![DbValue::Int(42)])
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);
}

#[tokio::test]
async fn test_execute_select_multiple_rows() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute(
            "CREATE TABLE IF NOT EXISTS t_multi (id INT PRIMARY KEY, val VARCHAR(50))",
        )
        .await
        .unwrap();
    driver
        .execute("INSERT INTO t_multi VALUES (1,'a'),(2,'b'),(3,'c')")
        .await
        .unwrap();

    let result = driver.execute("SELECT * FROM t_multi").await.unwrap();
    assert_eq!(result.rows.len(), 3);

    driver.execute("DROP TABLE t_multi").await.unwrap();
}

#[tokio::test]
async fn test_execute_insert_update_delete() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute("CREATE TABLE IF NOT EXISTS t_crud (id INT PRIMARY KEY, name VARCHAR(100))")
        .await
        .unwrap();

    // Insert
    let mut values = HashMap::new();
    values.insert("id".to_string(), DbValue::Int(1));
    values.insert("name".to_string(), DbValue::Text("Alice".to_string()));
    let affected = driver.insert_row("t_crud", values).await.unwrap();
    assert_eq!(affected, 1);

    // Update
    let mut pk = HashMap::new();
    pk.insert("id".to_string(), DbValue::Int(1));
    let mut changes = HashMap::new();
    changes.insert("name".to_string(), DbValue::Text("Bob".to_string()));
    let affected = driver.update_row("t_crud", pk.clone(), changes).await.unwrap();
    assert_eq!(affected, 1);

    // Delete
    let affected = driver.delete_row("t_crud", pk).await.unwrap();
    assert_eq!(affected, 1);

    driver.execute("DROP TABLE t_crud").await.unwrap();
}

#[tokio::test]
async fn test_load_schema() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute(
            "CREATE TABLE IF NOT EXISTS t_schema (\
                id INT PRIMARY KEY,\
                name VARCHAR(100) NOT NULL,\
                age INT\
             )",
        )
        .await
        .unwrap();

    let tree = driver.load_schema().await.unwrap();
    assert!(!tree.databases.is_empty());
    let schema = &tree.databases[0].schemas[0];
    let table = schema.tables.iter().find(|t| t.name == "t_schema");
    assert!(table.is_some(), "t_schema should be in schema tree");

    if let Some(t) = table {
        assert_eq!(t.columns.len(), 3);
    }

    driver.execute("DROP TABLE t_schema").await.unwrap();
}

#[tokio::test]
async fn test_table_data_pagination() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver.execute("CREATE TABLE IF NOT EXISTS t_pag (id INT PRIMARY KEY)")
        .await
        .unwrap();

    for i in 1..=10i64 {
        driver
            .execute_with_params(
                "INSERT INTO t_pag(id) VALUES(?)",
                vec![DbValue::Int(i)],
            )
            .await
            .unwrap();
    }

    let page0 = driver.table_data(None, "t_pag", 0, 5).await.unwrap();
    assert_eq!(page0.rows.len(), 5);

    let page1 = driver.table_data(None, "t_pag", 1, 5).await.unwrap();
    assert_eq!(page1.rows.len(), 5);

    let page2 = driver.table_data(None, "t_pag", 2, 5).await.unwrap();
    assert_eq!(page2.rows.len(), 0);

    driver.execute("DROP TABLE t_pag").await.unwrap();
}

#[tokio::test]
async fn test_execute_returns_execution_time() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver.execute("SELECT 1").await.unwrap();
    assert!(result.execution_time.as_nanos() > 0);
}

#[tokio::test]
async fn test_type_mapping_coverage() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Create a table with specific MySQL types to ensure correct type_info() names
    driver.execute(
        "CREATE TABLE IF NOT EXISTS t_types (\
            id INT PRIMARY KEY,\
            bool_col BOOLEAN,\
            tiny_col TINYINT,\
            small_col SMALLINT,\
            med_col MEDIUMINT,\
            big_col BIGINT,\
            float_col FLOAT,\
            double_col DOUBLE,\
            dec_col DECIMAL(10,2),\
            blob_col BLOB,\
            bin_col BINARY(4),\
            vbin_col VARBINARY(10),\
            json_col JSON,\
            dt_col DATETIME,\
            ts_col TIMESTAMP,\
            txt_col TEXT,\
            vc_col VARCHAR(50)\
         )",
    ).await.unwrap();

    driver.execute(
        "INSERT INTO t_types VALUES (\
            1, TRUE, 42, 100, 1000, 999999,\
            1.5, 2.71, 3.14,\
            0x0102, 0x0304, 0x0506,\
            '{\"x\":1}',\
            NOW(), NOW(),\
            'hello', 'world'\
         )",
    ).await.unwrap();

    let result = driver.execute("SELECT * FROM t_types WHERE id = 1").await.unwrap();
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.columns.len(), 17);

    driver.execute("DROP TABLE t_types").await.unwrap();
}

#[tokio::test]
async fn test_execute_error_paths() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Execute invalid SQL — triggers .map_err in execute()
    let err = driver.execute("INVALID SQL STATEMENT !!!").await;
    assert!(err.is_err());

    // Execute with params on bad SQL
    let err2 = driver
        .execute_with_params("SELECT ? FROM nonexistent_xyz", vec![DbValue::Int(1)])
        .await;
    assert!(err2.is_err());

    // table_data on nonexistent table
    let err3 = driver.table_data(None, "nonexistent_xyz", 0, 10).await;
    assert!(err3.is_err());

    // insert_row on nonexistent table
    let mut vals = HashMap::new();
    vals.insert("id".to_string(), DbValue::Int(1));
    let err4 = driver.insert_row("nonexistent_xyz", vals).await;
    assert!(err4.is_err());

    // update_row on nonexistent table
    let mut pk = HashMap::new();
    pk.insert("id".to_string(), DbValue::Int(1));
    let mut changes = HashMap::new();
    changes.insert("v".to_string(), DbValue::Int(2));
    let err5 = driver.update_row("nonexistent_xyz", pk.clone(), changes).await;
    assert!(err5.is_err());

    // delete_row on nonexistent table
    let err6 = driver.delete_row("nonexistent_xyz", pk).await;
    assert!(err6.is_err());
}
