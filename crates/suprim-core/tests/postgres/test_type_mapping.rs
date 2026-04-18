use crate::helpers;
use suprim_core::db::driver::DatabaseDriver;
use suprim_core::db::values::DbValue;

// ── Numeric types ────────────────────────────────────────────────────────────

#[tokio::test]
async fn bool_maps_to_bool() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT true::BOOL AS val").await.unwrap();
    assert!(matches!(result.rows[0][0], DbValue::Bool(true)));
}

#[tokio::test]
async fn int2_maps_to_int() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT 42::INT2 AS val").await.unwrap();
    assert!(matches!(result.rows[0][0], DbValue::Int(42)));
}

#[tokio::test]
async fn int4_maps_to_int() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT 100::INT4 AS val").await.unwrap();
    assert!(matches!(result.rows[0][0], DbValue::Int(100)));
}

#[tokio::test]
async fn int8_maps_to_int() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT 9999999::INT8 AS val").await.unwrap();
    assert!(matches!(result.rows[0][0], DbValue::Int(9999999)));
}

#[tokio::test]
async fn float4_maps_to_float() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT 1.5::FLOAT4 AS val").await.unwrap();
    match &result.rows[0][0] {
        DbValue::Float(v) => assert!((v - 1.5).abs() < 0.001),
        other => panic!("Expected Float, got {:?}", other),
    }
}

#[tokio::test]
async fn float8_maps_to_float() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT 2.71828::FLOAT8 AS val").await.unwrap();
    match &result.rows[0][0] {
        DbValue::Float(v) => assert!((v - 2.71828).abs() < 0.0001),
        other => panic!("Expected Float, got {:?}", other),
    }
}

#[tokio::test]
async fn numeric_maps_to_decimal() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT 12345.67::NUMERIC AS val").await.unwrap();
    match &result.rows[0][0] {
        DbValue::Decimal(s) => {
            let v: f64 = s.parse().unwrap();
            assert!((v - 12345.67).abs() < 0.01);
        }
        other => panic!("Expected Decimal, got {:?}", other),
    }
}

// ── String types ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn text_maps_to_text() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT 'hello'::TEXT AS val").await.unwrap();
    assert!(matches!(&result.rows[0][0], DbValue::Text(s) if s == "hello"));
}

#[tokio::test]
async fn varchar_maps_to_text() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT 'world'::VARCHAR(50) AS val").await.unwrap();
    assert!(matches!(&result.rows[0][0], DbValue::Text(s) if s == "world"));
}

#[tokio::test]
async fn char_maps_to_text() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT 'A'::CHAR(1) AS val").await.unwrap();
    assert!(matches!(&result.rows[0][0], DbValue::Text(s) if s == "A"));
}

// ── Binary ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn bytea_maps_to_bytes() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT '\\x0102'::BYTEA AS val").await.unwrap();
    assert!(matches!(&result.rows[0][0], DbValue::Bytes(b) if b == &[1, 2]));
}

// ── JSON ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn json_maps_to_json() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT '{\"k\":1}'::JSON AS val").await.unwrap();
    assert!(matches!(&result.rows[0][0], DbValue::Json(_)));
}

#[tokio::test]
async fn jsonb_maps_to_json() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT '{\"k\":2}'::JSONB AS val").await.unwrap();
    assert!(matches!(&result.rows[0][0], DbValue::Json(_)));
}

// ── Timestamps ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn timestamptz_maps_to_timestamp() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT NOW()::TIMESTAMPTZ AS val").await.unwrap();
    assert!(matches!(&result.rows[0][0], DbValue::Timestamp(_)));
}

#[tokio::test]
async fn timestamp_maps_to_timestamp() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT NOW()::TIMESTAMP AS val").await.unwrap();
    assert!(matches!(&result.rows[0][0], DbValue::Timestamp(_)));
}

// ── UUID ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn uuid_maps_to_text() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver
        .execute("SELECT '550e8400-e29b-41d4-a716-446655440000'::UUID AS val")
        .await
        .unwrap();
    assert!(matches!(&result.rows[0][0], DbValue::Text(s) if s == "550e8400-e29b-41d4-a716-446655440000"));
}

// ── Date / Time / Interval (now natively decoded) ────────────────────────────

#[tokio::test]
async fn date_maps_to_text() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT '2026-04-18'::DATE AS val").await.unwrap();
    assert!(
        matches!(&result.rows[0][0], DbValue::Text(s) if s == "2026-04-18"),
        "DATE should map to Text('2026-04-18'), got {:?}", result.rows[0][0]
    );
}

#[tokio::test]
async fn time_maps_to_text() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT '14:30:00'::TIME AS val").await.unwrap();
    assert!(
        matches!(&result.rows[0][0], DbValue::Text(s) if s.contains("14:30:00")),
        "TIME should map to Text containing '14:30:00', got {:?}", result.rows[0][0]
    );
}

#[tokio::test]
async fn interval_maps_to_text() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT '1 day 2 hours'::INTERVAL AS val").await.unwrap();
    assert!(
        matches!(&result.rows[0][0], DbValue::Text(_)),
        "INTERVAL should map to Text, got {:?}", result.rows[0][0]
    );
}

// ── OID (now natively decoded as Int) ────────────────────────────────────────

#[tokio::test]
async fn oid_maps_to_int() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver.execute("SELECT 1::OID AS val").await.unwrap();
    assert!(
        matches!(&result.rows[0][0], DbValue::Int(1)),
        "OID should map to Int(1), got {:?}", result.rows[0][0]
    );
}

// ── NULL handling ────────────────────────────────────────────────────────────

#[tokio::test]
async fn null_value_maps_to_null() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("public"), "users", 0, 1, Some("name = 'Charlie'"), None)
        .await
        .unwrap();

    let metadata_idx = result.columns.iter().position(|c| c.name == "metadata").unwrap();
    assert!(matches!(result.rows[0][metadata_idx], DbValue::Null), "NULL should map to Null");
}

// ── Table data type mapping (via real table) ─────────────────────────────────

#[tokio::test]
async fn bigint_via_serial_maps_to_int() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("public"), "users", 0, 1, Some("name = 'Alice'"), None)
        .await
        .unwrap();

    // id column (SERIAL = INT4 with sequence)
    assert!(matches!(result.rows[0][0], DbValue::Int(_)), "SERIAL/INT should map to Int");
}

#[tokio::test]
async fn varchar_from_table_maps_to_text() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("public"), "users", 0, 1, Some("name = 'Alice'"), None)
        .await
        .unwrap();

    let name_idx = result.columns.iter().position(|c| c.name == "name").unwrap();
    assert!(
        matches!(&result.rows[0][name_idx], DbValue::Text(s) if s == "Alice"),
        "VARCHAR should map to Text"
    );
}

#[tokio::test]
async fn jsonb_from_table() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("public"), "users", 0, 1, Some("name = 'Alice'"), None)
        .await
        .unwrap();

    // metadata column (JSONB)
    let metadata_idx = result.columns.iter().position(|c| c.name == "metadata").unwrap();
    assert!(
        matches!(&result.rows[0][metadata_idx], DbValue::Json(_) | DbValue::Null),
        "JSONB should map to Json or Null"
    );
}

#[tokio::test]
async fn decimal_from_table() {
    let driver = helpers::connected_driver("testdb").await;

    let result = driver
        .execute("SELECT salary FROM users WHERE name = 'Alice'")
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 1);
    match &result.rows[0][0] {
        DbValue::Decimal(s) => {
            let v: f64 = s.parse().expect("Decimal string should parse to f64");
            assert!((v - 75000.5).abs() < 0.01, "DECIMAL should be ~75000.50, got {}", v);
        }
        other => panic!("DECIMAL should map to Decimal, got {:?}", other),
    }
}
