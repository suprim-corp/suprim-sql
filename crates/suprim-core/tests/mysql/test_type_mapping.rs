use crate::helpers;
use suprim_core::db::driver::DatabaseDriver;
use suprim_core::db::values::DbValue;

#[tokio::test]
async fn bigint_maps_to_int() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 1, Some("name = 'Alice'"), None)
        .await
        .unwrap();

    // id column (BIGINT AUTO_INCREMENT)
    assert!(matches!(result.rows[0][0], DbValue::Int(_)), "BIGINT should map to Int");
}

#[tokio::test]
async fn varchar_maps_to_text() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 1, Some("name = 'Alice'"), None)
        .await
        .unwrap();

    // name column (VARCHAR)
    assert!(matches!(&result.rows[0][1], DbValue::Text(s) if s == "Alice"),
        "VARCHAR should map to Text");
}

#[tokio::test]
async fn int_maps_to_int() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 1, Some("name = 'Alice'"), None)
        .await
        .unwrap();

    // age column (INT)
    assert!(matches!(result.rows[0][3], DbValue::Int(30)), "INT should map to Int(30)");
}

#[tokio::test]
async fn boolean_maps_to_bool_or_int() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 1, Some("name = 'Alice'"), None)
        .await
        .unwrap();

    // active column (BOOLEAN = TINYINT(1))
    assert!(
        matches!(result.rows[0][4], DbValue::Bool(true) | DbValue::Int(1)),
        "BOOLEAN should map to Bool(true) or Int(1)"
    );
}

#[tokio::test]
async fn decimal_maps_to_decimal() {
    let driver = helpers::connected_driver("testdb").await;

    let result = driver
        .execute("SELECT salary FROM users WHERE name = 'Alice'")
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 1);
    let salary = &result.rows[0][0];

    match salary {
        DbValue::Decimal(s) => {
            let v: f64 = s.parse().expect("Decimal string should parse to f64");
            assert!((v - 75000.5).abs() < 0.01, "DECIMAL should be ~75000.50, got {}", v);
        }
        other => panic!("DECIMAL should map to Decimal, got {:?} (col type: {})",
            other, result.columns[0].db_type),
    }
}

#[tokio::test]
async fn json_maps_to_json_or_text() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 1, Some("name = 'Alice'"), None)
        .await
        .unwrap();

    // metadata column (JSON)
    assert!(
        matches!(&result.rows[0][6], DbValue::Json(_) | DbValue::Text(_)),
        "JSON should map to Json or Text"
    );
}

#[tokio::test]
async fn null_value_maps_to_null() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 1, Some("name = 'Charlie'"), None)
        .await
        .unwrap();

    // Charlie's metadata is NULL
    let metadata_idx = result.columns.iter().position(|c| c.name == "metadata").unwrap();
    assert!(matches!(result.rows[0][metadata_idx], DbValue::Null), "NULL should map to Null");
}

#[tokio::test]
async fn timestamp_maps_to_timestamp_or_text() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 1, Some("name = 'Alice'"), None)
        .await
        .unwrap();

    // created_at column (TIMESTAMP)
    let ts_idx = result.columns.iter().position(|c| c.name == "created_at").unwrap();
    assert!(
        matches!(&result.rows[0][ts_idx], DbValue::Timestamp(_) | DbValue::Text(_)),
        "TIMESTAMP should map to Timestamp or Text"
    );
}
