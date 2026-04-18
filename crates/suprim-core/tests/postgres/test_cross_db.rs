use crate::helpers;
use suprim_core::db::driver::DatabaseDriver;

#[tokio::test]
async fn execute_on_different_database() {
    let driver = helpers::connected_driver("testdb").await;

    // testdb2 has an `items` table with 2 rows
    let result = driver
        .execute_on_database("SELECT * FROM items", "testdb2")
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 2);
}

#[tokio::test]
async fn execute_on_database_then_original_still_works() {
    let driver = helpers::connected_driver("testdb").await;

    // Switch to testdb2
    let _ = driver
        .execute_on_database("SELECT 1", "testdb2")
        .await
        .unwrap();

    // Original testdb queries should still work (pool connection is separate)
    let result = driver.execute("SELECT COUNT(*) AS cnt FROM orders").await.unwrap();
    assert_eq!(result.rows.len(), 1);
}

#[tokio::test]
async fn list_schemas_on_different_database() {
    let driver = helpers::connected_driver("testdb").await;

    let schemas = driver.list_schemas("testdb2").await.unwrap();
    assert!(schemas.contains(&"public".to_string()));
}

#[tokio::test]
async fn load_schema_detail_on_different_database() {
    let driver = helpers::connected_driver("testdb").await;

    let schema = driver.load_schema_detail("testdb2", "public").await.unwrap();
    assert!(
        schema.tables.iter().any(|t| t.name == "items"),
        "testdb2 should have items table"
    );
}
