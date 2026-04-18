use crate::helpers;
use suprim_core::db::driver::DatabaseDriver;

#[tokio::test]
async fn basic_returns_all_rows() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 50, None, None)
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 5, "users table should have exactly 5 rows after reset");
    assert_eq!(result.total_count, Some(5));
    assert!(result.columns.len() >= 7);
}

#[tokio::test]
async fn pagination_count_and_offset() {
    let driver = helpers::connected_driver("testdb").await;

    // orders table has exactly 5 fixed rows
    let page0 = driver
        .table_data(Some("testdb"), Some("testdb"), "orders", 0, 2, None, None)
        .await
        .unwrap();
    assert_eq!(page0.rows.len(), 2);
    assert_eq!(page0.total_count, Some(5));

    let page1 = driver
        .table_data(Some("testdb"), Some("testdb"), "orders", 1, 2, None, None)
        .await
        .unwrap();
    assert_eq!(page1.rows.len(), 2);

    let page2 = driver
        .table_data(Some("testdb"), Some("testdb"), "orders", 2, 2, None, None)
        .await
        .unwrap();
    assert_eq!(page2.rows.len(), 1, "last page should have 1 row");
}

#[tokio::test]
async fn where_clause_filters_rows() {
    let driver = helpers::connected_driver("testdb").await;

    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "orders", 0, 50,
            Some("status = 'pending'"), None,
        )
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 2, "2 orders have status=pending");
    assert_eq!(result.total_count, Some(2));
}

#[tokio::test]
async fn order_clause_sorts_rows() {
    let driver = helpers::connected_driver("testdb").await;

    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "orders", 0, 50,
            None, Some("total DESC"),
        )
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 5);
    // Verify descending order — first row's total should be >= second
    let totals: Vec<f64> = result.rows.iter().filter_map(|r| {
        match &r[2] {
            suprim_core::db::values::DbValue::Float(v) => Some(*v),
            suprim_core::db::values::DbValue::Decimal(s) => s.parse().ok(),
            _ => None,
        }
    }).collect();
    for w in totals.windows(2) {
        assert!(w[0] >= w[1], "Expected descending order: {} >= {}", w[0], w[1]);
    }
}

#[tokio::test]
async fn where_and_order_combined() {
    let driver = helpers::connected_driver("testdb").await;

    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "orders", 0, 50,
            Some("total > 50"), Some("total ASC"),
        )
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 4, "4 orders have total > 50");
    assert_eq!(result.total_count, Some(4));
}

#[tokio::test]
async fn empty_where_clause_ignored() {
    let driver = helpers::connected_driver("testdb").await;

    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "orders", 0, 50,
            Some("  "), None,
        )
        .await
        .unwrap();

    assert_eq!(result.rows.len(), 5, "empty where should return all rows");
}

#[tokio::test]
async fn no_database_param_uses_connection_default() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver
        .table_data(None, None, "orders", 0, 50, None, None)
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 5);
}

#[tokio::test]
async fn page_beyond_data_returns_empty() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "orders", 100, 50, None, None)
        .await
        .unwrap();
    assert_eq!(result.rows.len(), 0);
    assert_eq!(result.total_count, Some(5), "total_count should still reflect full table");
}

#[tokio::test]
async fn nullable_metadata_correct() {
    let driver = helpers::connected_driver("testdb").await;
    let result = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 1, None, None)
        .await
        .unwrap();

    let name_col = result.columns.iter().find(|c| c.name == "name").unwrap();
    assert!(!name_col.nullable, "name is NOT NULL");

    let email_col = result.columns.iter().find(|c| c.name == "email").unwrap();
    assert!(email_col.nullable, "email allows NULL");

    let metadata_col = result.columns.iter().find(|c| c.name == "metadata").unwrap();
    assert!(metadata_col.nullable, "metadata allows NULL");
}
