//! Integration tests for the PostgreSQL driver.
//! Requires Docker — uses testcontainers-rs to spin up postgres:15.

use std::collections::HashMap;

use testcontainers::runners::AsyncRunner;
use testcontainers_modules::postgres::Postgres;

use suprim_core::db::{ConnectionConfig, DbFactory, DbValue, DriverParams};

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn setup() -> (ConnectionConfig, impl Drop) {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();

    let config = ConnectionConfig::new(
        "test-postgres",
        DriverParams::Postgres {
            host: "127.0.0.1".into(),
            port,
            database: "postgres".into(),
            user: "postgres".into(),
            // testcontainers default password
            password_key: "postgres".into(),
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
    driver.disconnect().await.unwrap();
    assert!(!driver.is_connected());
}

#[tokio::test]
async fn test_ping_ok() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();
    driver.ping().await.unwrap();
}

#[tokio::test]
async fn test_connect_wrong_password() {
    let (config, _container) = setup().await;

    let bad_config = ConnectionConfig::new(
        "bad",
        DriverParams::Postgres {
            host: match &config.params {
                DriverParams::Postgres { host, .. } => host.clone(),
                _ => unreachable!(),
            },
            port: match &config.params {
                DriverParams::Postgres { port, .. } => *port,
                _ => unreachable!(),
            },
            database: "postgres".into(),
            user: "postgres".into(),
            password_key: "wrong_password".into(),
        },
    );

    let mut driver = DbFactory::create(&bad_config).unwrap();
    let result = driver.connect(&bad_config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_execute_select() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver.execute("SELECT 1 AS num, 'hello' AS txt").await.unwrap();
    assert_eq!(result.columns.len(), 2);
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.columns[0].name, "num");
    assert_eq!(result.columns[1].name, "txt");
}

#[tokio::test]
async fn test_execute_select_multiple_rows() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver
        .execute(
            "SELECT generate_series AS n \
             FROM generate_series(1, 5)",
        )
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 5);
}

#[tokio::test]
async fn test_execute_with_params() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    use suprim_core::db::DbValue;
    let result = driver
        .execute_with_params(
            "SELECT $1::text AS val",
            vec![DbValue::Text("world".into())],
        )
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0][0], DbValue::Text("world".into()));
}

#[tokio::test]
async fn test_execute_insert_update_delete() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Setup
    driver
        .execute(
            "CREATE TABLE test_iud (id SERIAL PRIMARY KEY, name TEXT NOT NULL)",
        )
        .await
        .unwrap();

    // INSERT
    let mut values = HashMap::new();
    values.insert("name".to_string(), suprim_core::db::DbValue::Text("Alice".into()));
    let inserted = driver.insert_row("test_iud", values).await.unwrap();
    assert_eq!(inserted, 1);

    // Verify row exists
    let result = driver.execute("SELECT * FROM test_iud").await.unwrap();
    assert_eq!(result.rows.len(), 1);

    // UPDATE
    let mut pk = HashMap::new();
    pk.insert("id".to_string(), suprim_core::db::DbValue::Int(1));
    let mut changes = HashMap::new();
    changes.insert("name".to_string(), suprim_core::db::DbValue::Text("Bob".into()));
    let updated = driver.update_row("test_iud", pk.clone(), changes).await.unwrap();
    assert_eq!(updated, 1);

    // Verify update
    let result = driver.execute("SELECT name FROM test_iud WHERE id = 1").await.unwrap();
    assert_eq!(result.rows[0][0], suprim_core::db::DbValue::Text("Bob".into()));

    // DELETE
    let deleted = driver.delete_row("test_iud", pk).await.unwrap();
    assert_eq!(deleted, 1);

    // Verify empty
    let result = driver.execute("SELECT * FROM test_iud").await.unwrap();
    assert_eq!(result.rows.len(), 0);
}

#[tokio::test]
async fn test_table_data_pagination() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute("CREATE TABLE test_pag (id SERIAL PRIMARY KEY, val TEXT)")
        .await
        .unwrap();
    driver
        .execute("INSERT INTO test_pag (val) SELECT 'row'||n FROM generate_series(1,10) n")
        .await
        .unwrap();

    let page0 = driver
        .table_data(None, "test_pag", 0, 5)
        .await
        .unwrap();
    assert_eq!(page0.rows.len(), 5);

    let page1 = driver
        .table_data(None, "test_pag", 1, 5)
        .await
        .unwrap();
    assert_eq!(page1.rows.len(), 5);

    // Page beyond data returns empty
    let page2 = driver
        .table_data(None, "test_pag", 2, 5)
        .await
        .unwrap();
    assert_eq!(page2.rows.len(), 0);
}

#[tokio::test]
async fn test_load_schema() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute(
            "CREATE TABLE schema_test ( \
                 id SERIAL PRIMARY KEY, \
                 name TEXT NOT NULL, \
                 age INT \
             )",
        )
        .await
        .unwrap();

    let tree = driver.load_schema().await.unwrap();
    assert!(!tree.databases.is_empty());

    let db = tree.databases.iter().find(|d| d.name == "postgres").unwrap();
    let schema = db.schemas.iter().find(|s| s.name == "public").unwrap();
    let table = schema.tables.iter().find(|t| t.name == "schema_test").unwrap();

    assert_eq!(table.columns.len(), 3);
    let id_col = table.columns.iter().find(|c| c.name == "id").unwrap();
    assert!(id_col.is_primary_key);
    let name_col = table.columns.iter().find(|c| c.name == "name").unwrap();
    assert!(!name_col.nullable);
    let age_col = table.columns.iter().find(|c| c.name == "age").unwrap();
    assert!(age_col.nullable);
}

#[tokio::test]
async fn test_load_schema_with_index() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute("CREATE TABLE idx_test (id SERIAL PRIMARY KEY, email TEXT UNIQUE)")
        .await
        .unwrap();
    driver
        .execute("CREATE INDEX idx_email ON idx_test(email)")
        .await
        .unwrap();

    let tree = driver.load_schema().await.unwrap();
    let db = tree.databases.iter().find(|d| d.name == "postgres").unwrap();
    let schema = db.schemas.iter().find(|s| s.name == "public").unwrap();
    let table = schema.tables.iter().find(|t| t.name == "idx_test").unwrap();

    assert!(!table.indexes.is_empty());
}

#[tokio::test]
async fn test_load_schema_with_foreign_key() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    driver
        .execute("CREATE TABLE parent (id SERIAL PRIMARY KEY)")
        .await
        .unwrap();
    driver
        .execute("CREATE TABLE child (id SERIAL PRIMARY KEY, parent_id INT REFERENCES parent(id))")
        .await
        .unwrap();

    let tree = driver.load_schema().await.unwrap();
    let db = tree.databases.iter().find(|d| d.name == "postgres").unwrap();
    let schema = db.schemas.iter().find(|s| s.name == "public").unwrap();
    let child = schema.tables.iter().find(|t| t.name == "child").unwrap();

    assert!(!child.foreign_keys.is_empty());
    assert_eq!(child.foreign_keys[0].ref_table, "parent");
}

#[tokio::test]
async fn test_execute_returns_execution_time() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    let result = driver.execute("SELECT 1").await.unwrap();
    // Execution time should be non-zero
    assert!(result.execution_time.as_nanos() > 0);
}

#[tokio::test]
async fn test_type_mapping_coverage() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Cover pg_value_from_row type branches: BOOL, INT2, INT4, INT8, FLOAT4, FLOAT8
    let result = driver
        .execute(
            "SELECT true::BOOL as b, \
                    1::int2 as i2, 100::int4 as i4, 9999::int8 as i8, \
                    1.5::float4 as f4, 2.71::float8 as f8",
        )
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.columns.len(), 6);

    // Cover TEXT, VARCHAR, BYTEA
    let result2 = driver
        .execute(
            "SELECT 'hello'::TEXT as t, 'world'::VARCHAR(10) as v, \
                    'x'::CHAR(1) as c, '\\x0102'::BYTEA as by",
        )
        .await
        .unwrap();
    assert_eq!(result2.rows.len(), 1);
    assert_eq!(result2.columns.len(), 4);

    // Cover JSON, JSONB
    let result3 = driver
        .execute(
            "SELECT '{\"k\":1}'::JSON as j, '{\"k\":2}'::JSONB as jb",
        )
        .await
        .unwrap();
    assert_eq!(result3.rows.len(), 1);

    // Cover TIMESTAMPTZ, TIMESTAMP
    let result4 = driver
        .execute("SELECT NOW()::TIMESTAMPTZ as tz, NOW()::TIMESTAMP as ts")
        .await
        .unwrap();
    assert_eq!(result4.rows.len(), 1);

    // Cover UUID — cast string literal (no extension needed)
    let result5 = driver
        .execute("SELECT '550e8400-e29b-41d4-a716-446655440000'::UUID as uuid_val")
        .await
        .unwrap();
    assert_eq!(result5.rows.len(), 1);

    // Cover pg_value_from_row fallback arm — use OID type (integer OID, not in match arms)
    let result6 = driver
        .execute("SELECT 1::OID as oid_val, NOW()::TIME as time_val")
        .await
        .unwrap();
    assert_eq!(result6.rows.len(), 1);
}

#[tokio::test]
async fn test_execute_error_paths() {
    let (config, _container) = setup().await;
    let mut driver = DbFactory::create(&config).unwrap();
    driver.connect(&config).await.unwrap();

    // Execute with invalid SQL — triggers .map_err in execute()
    let err = driver.execute("INVALID SQL STATEMENT !!!").await;
    assert!(err.is_err());

    // Execute with params on bad SQL — triggers .map_err in execute_with_params()
    let err2 = driver
        .execute_with_params("SELECT $1 FROM nonexistent_xyz", vec![DbValue::Int(1)])
        .await;
    assert!(err2.is_err());

    // table_data on nonexistent table — triggers .map_err in table_data()
    let err3 = driver.table_data(None, "nonexistent_xyz", 0, 10).await;
    assert!(err3.is_err());

    // insert_row on nonexistent table — triggers .map_err in insert_row()
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
