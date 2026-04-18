use crate::helpers;
use suprim_core::db::driver::DatabaseDriver;

#[tokio::test]
async fn list_databases() {
    let driver = helpers::connected_driver("testdb").await;
    let dbs = driver.list_databases().await.unwrap();
    assert!(dbs.contains(&"testdb".to_string()));
    assert!(dbs.contains(&"postgres".to_string()));
}

#[tokio::test]
async fn list_schemas_returns_public() {
    let driver = helpers::connected_driver("testdb").await;
    let schemas = driver.list_schemas("testdb").await.unwrap();
    assert!(schemas.contains(&"public".to_string()));
}

#[tokio::test]
async fn load_schema_detail_tables() {
    let driver = helpers::connected_driver("testdb").await;
    let schema = driver.load_schema_detail("testdb", "public").await.unwrap();

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
    assert!(
        users.indexes.iter().any(|i| i.columns.contains(&"email".to_string())),
        "Should have an index on email column"
    );
}

#[tokio::test]
async fn load_schema_detail_foreign_keys() {
    let driver = helpers::connected_driver("testdb").await;
    let schema = driver.load_schema_detail("testdb", "public").await.unwrap();

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
    let schema = driver.load_schema_detail("testdb", "public").await.unwrap();
    assert!(
        schema.views.iter().any(|v| v.name == "active_users"),
        "Should find active_users view"
    );
}

#[tokio::test]
async fn load_schema_detail_materialized_views() {
    let driver = helpers::connected_driver("testdb").await;
    let schema = driver.load_schema_detail("testdb", "public").await.unwrap();
    assert!(
        schema.materialized_views.iter().any(|v| v.name == "user_stats"),
        "Should find user_stats materialized view"
    );
}

#[tokio::test]
async fn load_schema_detail_sequences() {
    let driver = helpers::connected_driver("testdb").await;
    let schema = driver.load_schema_detail("testdb", "public").await.unwrap();
    // users_id_seq from SERIAL column
    assert!(
        schema.sequences.iter().any(|s| s.name.contains("users_id_seq")),
        "Should find users_id_seq sequence, got: {:?}",
        schema.sequences.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn load_schema_detail_functions() {
    let driver = helpers::connected_driver("testdb").await;
    let schema = driver.load_schema_detail("testdb", "public").await.unwrap();
    assert!(
        schema.functions.iter().any(|f| f.name == "get_user_count"),
        "Should find get_user_count function"
    );
}

#[tokio::test]
async fn empty_schema_returns_empty() {
    let driver = helpers::connected_driver("testdb").await;
    // Create a new schema that has nothing
    let _ = driver.execute("DROP SCHEMA IF EXISTS empty_test_schema CASCADE").await;
    driver.execute("CREATE SCHEMA empty_test_schema").await.unwrap();

    let schema = driver.load_schema_detail("testdb", "empty_test_schema").await.unwrap();
    assert!(schema.tables.is_empty());
    assert!(schema.views.is_empty());
    assert!(schema.functions.is_empty());

    driver.execute("DROP SCHEMA empty_test_schema").await.unwrap();
}

#[tokio::test]
async fn list_extensions() {
    let driver = helpers::connected_driver("testdb").await;
    // list_extensions should not crash — may be empty or have pg_stat_statements
    let result = driver.list_extensions("testdb").await;
    assert!(result.is_ok());
}

// ── DDL generation dialect test ──────────────────────────────────────────────

#[tokio::test]
async fn ddl_generator_uses_double_quotes_for_postgres() {
    use suprim_core::db::ddl_generator;
    use suprim_core::db::dialect::SqlDialect;

    let driver = helpers::connected_driver("testdb").await;
    let schema = driver.load_schema_detail("testdb", "public").await.unwrap();

    let users = schema.tables.iter().find(|t| t.name == "users").expect("users table");
    let ddl = ddl_generator::full_table_ddl("public", users, SqlDialect::Postgres);

    // Must use double-quote quoting
    assert!(ddl.contains("\"users\""), "Table name should use double quotes: {ddl}");
    assert!(ddl.contains("\"id\""), "Column names should use double quotes: {ddl}");
    // Must NOT use backtick quoting
    assert!(!ddl.contains("`users`"), "Should not use MySQL backticks: {ddl}");
    // Should have PRIMARY KEY
    assert!(ddl.contains("PRIMARY KEY"), "Should have PK constraint: {ddl}");
}
