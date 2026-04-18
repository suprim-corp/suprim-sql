use crate::helpers;
use suprim_core::db::driver::DatabaseDriver;

/// Verify that mutation attempts via WHERE clause injection are blocked
/// by the READ ONLY session in table_data().

#[tokio::test]
async fn where_injection_drop_table_blocked() {
    let driver = helpers::connected_driver("testdb").await;

    // Create a sacrificial table
    driver.execute("CREATE TABLE IF NOT EXISTS injection_target (id INT PRIMARY KEY)").await.unwrap();
    driver.execute("INSERT IGNORE INTO injection_target VALUES (1)").await.unwrap();

    // Attempt injection via WHERE clause — should fail because session is READ ONLY
    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            Some("1=1; DROP TABLE injection_target; --"), None,
        )
        .await;

    // The query should error (syntax error or READ ONLY violation), NOT silently succeed
    // Either way, the table must still exist
    let check = driver.execute("SELECT COUNT(*) AS cnt FROM injection_target").await;
    assert!(check.is_ok(), "injection_target should still exist after injection attempt");

    // Cleanup
    driver.execute("DROP TABLE IF EXISTS injection_target").await.unwrap();
}

#[tokio::test]
async fn where_injection_update_blocked() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    // Attempt to UPDATE via WHERE injection — READ ONLY should block
    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            Some("1=1; UPDATE users SET name='HACKED'; --"), None,
        )
        .await;

    // Verify no data was modified
    let check = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 50,
            Some("name = 'HACKED'"), None)
        .await
        .unwrap();
    assert_eq!(check.rows.len(), 0, "No rows should have been modified by injection");
}

#[tokio::test]
async fn where_injection_delete_blocked() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let before = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 50, None, None)
        .await
        .unwrap();
    let count_before = before.total_count.unwrap_or(0);

    // Attempt DELETE via WHERE injection
    let _ = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            Some("1=1; DELETE FROM users; --"), None,
        )
        .await;

    // Verify no rows deleted
    let after = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 50, None, None)
        .await
        .unwrap();
    assert_eq!(after.total_count.unwrap_or(0), count_before,
        "Row count should be unchanged after injection attempt");
}

#[tokio::test]
async fn where_injection_insert_blocked() {
    let driver = helpers::connected_driver("testdb").await;
    helpers::reset_users_table(&driver).await;

    let before = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 50, None, None)
        .await
        .unwrap();

    // Attempt INSERT via WHERE injection
    let _ = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            Some("1=1; INSERT INTO users (name,email) VALUES ('HACKED','hack@evil.com'); --"), None,
        )
        .await;

    let after = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 50, None, None)
        .await
        .unwrap();
    assert_eq!(after.total_count, before.total_count,
        "No rows should have been inserted by injection");
}

#[tokio::test]
async fn order_injection_subquery_blocked() {
    let driver = helpers::connected_driver("testdb").await;

    // Attempt to inject subquery via ORDER BY — should error or be harmless
    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            None, Some("(SELECT password FROM mysql.user LIMIT 1)"),
        )
        .await;

    // READ ONLY doesn't protect ORDER BY injection for data extraction,
    // but MySQL should reject the invalid ORDER BY syntax
    // The key assertion: our users table is untouched regardless
    let check = driver
        .table_data(Some("testdb"), Some("testdb"), "users", 0, 50, None, None)
        .await
        .unwrap();
    assert!(check.rows.len() >= 5, "Users table should be intact");
}

#[tokio::test]
async fn where_union_injection_data_leak() {
    let driver = helpers::connected_driver("testdb").await;

    // UNION injection — READ ONLY allows reads, so this tests whether
    // the query structure prevents UNION-based data extraction.
    // With READ ONLY, the query won't mutate data, but UNION SELECT could
    // return sensitive data from other tables.
    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            Some("1=0 UNION SELECT host,user,authentication_string,4,5,6,7,8 FROM mysql.user; --"), None,
        )
        .await;

    // This may succeed (READ ONLY allows reads) or fail (column count mismatch).
    // Document: READ ONLY does NOT prevent data leaks via UNION injection.
    // The WHERE/ORDER BY filter bar is for trusted user input (the app operator),
    // not for untrusted external input.
    //
    // If it succeeds, verify we at least didn't crash:
    if let Ok(r) = &result {
        // The result may contain leaked data — this is a known limitation.
        // Application-level mitigation: the filter bar is only accessible to
        // the authenticated database user who already has SELECT access.
        let _ = r;
    }
    // If it fails, that's also acceptable (column count mismatch protection).
}
