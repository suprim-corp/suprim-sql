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

    // Subquery in ORDER BY — blocked by sanitizer
    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            None, Some("(SELECT password FROM mysql.user LIMIT 1)"),
        )
        .await;

    assert!(result.is_err(), "Subquery in ORDER BY should be rejected by sanitizer");
}

#[tokio::test]
async fn where_union_injection_blocked() {
    let driver = helpers::connected_driver("testdb").await;

    // UNION injection — blocked by sanitizer (UNION keyword rejected)
    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            Some("1=0 UNION SELECT host,user,authentication_string,4,5,6,7,8 FROM mysql.user"), None,
        )
        .await;

    assert!(result.is_err(), "UNION in WHERE should be rejected by sanitizer");
}

#[tokio::test]
async fn mysql_hash_comment_blocked() {
    let driver = helpers::connected_driver("testdb").await;

    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            Some("1=1 # comment to hide injection"), None,
        )
        .await;

    assert!(result.is_err(), "MySQL # comment should be rejected");
}

#[tokio::test]
async fn sleep_injection_blocked() {
    let driver = helpers::connected_driver("testdb").await;

    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            Some("1=1 AND SLEEP(5)"), None,
        )
        .await;

    assert!(result.is_err(), "SLEEP() should be rejected by sanitizer");
}

#[tokio::test]
async fn load_file_injection_blocked() {
    let driver = helpers::connected_driver("testdb").await;

    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            Some("1=0 UNION SELECT LOAD_FILE('/etc/passwd')"), None,
        )
        .await;

    assert!(result.is_err(), "LOAD_FILE should be rejected by sanitizer");
}

#[tokio::test]
async fn into_outfile_blocked() {
    let driver = helpers::connected_driver("testdb").await;

    let result = driver
        .table_data(
            Some("testdb"), Some("testdb"), "users", 0, 50,
            Some("1=1 INTO OUTFILE '/tmp/dump.csv'"), None,
        )
        .await;

    assert!(result.is_err(), "INTO OUTFILE should be rejected by sanitizer");
}
