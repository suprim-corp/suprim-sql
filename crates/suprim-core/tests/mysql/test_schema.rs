use crate::helpers;
use suprim_core::db::driver::DatabaseDriver;

#[tokio::test]
async fn list_databases() {
    let driver = helpers::connected_driver("testdb").await;
    let dbs = driver.list_databases().await.unwrap();
    assert!(dbs.contains(&"testdb".to_string()));
    assert!(dbs.contains(&"information_schema".to_string()));
}

#[tokio::test]
async fn list_schemas_returns_database_name() {
    let driver = helpers::connected_driver("testdb").await;
    let schemas = driver.list_schemas("testdb").await.unwrap();
    assert_eq!(schemas, vec!["testdb"]);
}

#[tokio::test]
async fn load_schema_detail_tables() {
    let driver = helpers::connected_driver("testdb").await;
    let schema = driver.load_schema_detail("testdb", "testdb").await.unwrap();

    assert!(schema.tables.len() >= 2, "Expected at least users + orders");
    let users = schema.tables.iter().find(|t| t.name == "users").expect("users table");
    assert!(users.columns.len() >= 7);

    // PK
    let id_col = users.columns.iter().find(|c| c.name == "id").unwrap();
    assert!(id_col.is_primary_key);
    assert!(!id_col.nullable);

    // Nullable
    let email_col = users.columns.iter().find(|c| c.name == "email").unwrap();
    assert!(email_col.nullable);

    // Index
    assert!(users.indexes.iter().any(|i| i.name == "idx_users_email"));
}

#[tokio::test]
async fn load_schema_detail_foreign_keys() {
    let driver = helpers::connected_driver("testdb").await;
    let schema = driver.load_schema_detail("testdb", "testdb").await.unwrap();

    let orders = schema.tables.iter().find(|t| t.name == "orders").expect("orders table");
    assert!(!orders.foreign_keys.is_empty());
    let fk = &orders.foreign_keys[0];
    assert_eq!(fk.ref_table, "users");
    assert!(fk.columns.contains(&"user_id".to_string()));
    assert!(fk.ref_columns.contains(&"id".to_string()));
}

#[tokio::test]
async fn load_schema_detail_views() {
    let driver = helpers::connected_driver("testdb").await;
    let schema = driver.load_schema_detail("testdb", "testdb").await.unwrap();
    assert!(schema.views.iter().any(|v| v.name == "active_users"));
}

#[tokio::test]
async fn load_schema_detail_functions() {
    let driver = helpers::connected_driver("testdb").await;
    let schema = driver.load_schema_detail("testdb", "testdb").await.unwrap();
    assert!(schema.functions.iter().any(|f| f.name == "get_user_count"));
}

#[tokio::test]
async fn load_schema_detail_mysql_specifics() {
    let driver = helpers::connected_driver("testdb").await;
    let schema = driver.load_schema_detail("testdb", "testdb").await.unwrap();

    // MySQL has no materialized views or sequences
    assert!(schema.materialized_views.is_empty());
    assert!(schema.sequences.is_empty());
}

#[tokio::test]
async fn empty_database_returns_empty_schema() {
    let driver = helpers::connected_driver("testdb").await;
    let _ = driver.execute("DROP DATABASE IF EXISTS empty_schema_test").await;
    driver.execute("CREATE DATABASE empty_schema_test").await.unwrap();

    let schema = driver.load_schema_detail("empty_schema_test", "empty_schema_test").await.unwrap();
    assert!(schema.tables.is_empty());
    assert!(schema.views.is_empty());
    assert!(schema.functions.is_empty());

    driver.execute("DROP DATABASE empty_schema_test").await.unwrap();
}
