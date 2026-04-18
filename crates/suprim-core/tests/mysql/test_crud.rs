use std::collections::HashMap;
use crate::helpers;
use suprim_core::db::driver::DatabaseDriver;
use suprim_core::db::values::DbValue;

#[tokio::test]
async fn insert_update_delete_lifecycle() {
    let driver = helpers::connected_driver("testdb").await;

    // Clean up from any previous failed run
    let _ = driver.execute("DELETE FROM users WHERE email = 'crud@test.com'").await;

    // INSERT
    let mut values = HashMap::new();
    values.insert("name".to_string(), DbValue::Text("CrudTest".to_string()));
    values.insert("email".to_string(), DbValue::Text("crud@test.com".to_string()));
    values.insert("age".to_string(), DbValue::Int(99));
    values.insert("active".to_string(), DbValue::Bool(true));

    let affected = driver.insert_row("users", values).await.unwrap();
    assert_eq!(affected, 1);

    // Verify INSERT
    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 50, Some("email = 'crud@test.com'"), None)
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);

    // UPDATE
    let mut pk = HashMap::new();
    pk.insert("email".to_string(), DbValue::Text("crud@test.com".to_string()));
    let mut changes = HashMap::new();
    changes.insert("name".to_string(), DbValue::Text("Updated".to_string()));
    changes.insert("age".to_string(), DbValue::Int(100));

    let affected = driver.update_row("users", pk.clone(), changes).await.unwrap();
    assert_eq!(affected, 1);

    // Verify UPDATE
    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 50, Some("email = 'crud@test.com'"), None)
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);

    // DELETE
    let affected = driver.delete_row("users", pk).await.unwrap();
    assert_eq!(affected, 1);

    // Verify DELETE
    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 50, Some("email = 'crud@test.com'"), None)
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 0);
}

#[tokio::test]
async fn insert_null_values() {
    let driver = helpers::connected_driver("testdb").await;
    let _ = driver.execute("DELETE FROM users WHERE email = 'null@test.com'").await;

    let mut values = HashMap::new();
    values.insert("name".to_string(), DbValue::Text("NullTest".to_string()));
    values.insert("email".to_string(), DbValue::Text("null@test.com".to_string()));
    values.insert("metadata".to_string(), DbValue::Null);

    let affected = driver.insert_row("users", values).await.unwrap();
    assert_eq!(affected, 1);

    // Verify NULL stored correctly
    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 50, Some("email = 'null@test.com'"), None)
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 1);
    // metadata column should be NULL
    let metadata_idx = result.columns.iter().position(|c| c.name == "metadata").unwrap();
    assert!(matches!(result.rows[0][metadata_idx], DbValue::Null));

    // Cleanup
    let mut pk = HashMap::new();
    pk.insert("email".to_string(), DbValue::Text("null@test.com".to_string()));
    driver.delete_row("users", pk).await.unwrap();
}
