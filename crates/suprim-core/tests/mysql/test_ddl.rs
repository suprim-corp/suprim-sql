use crate::helpers;
use suprim_core::db::driver::DatabaseDriver;

#[tokio::test]
async fn truncate_table() {
    let driver = helpers::connected_driver("testdb").await;
    driver.execute("CREATE TABLE IF NOT EXISTS ddl_truncate (id INT PRIMARY KEY)").await.unwrap();
    driver.execute("INSERT IGNORE INTO ddl_truncate VALUES (1),(2),(3)").await.unwrap();

    driver.truncate_table("testdb", "ddl_truncate").await.unwrap();

    let result = driver.execute("SELECT COUNT(*) AS cnt FROM ddl_truncate").await.unwrap();
    assert!(matches!(result.rows[0][0], suprim_core::db::values::DbValue::Int(0)));

    driver.execute("DROP TABLE IF EXISTS ddl_truncate").await.unwrap();
}

#[tokio::test]
async fn rename_table() {
    let driver = helpers::connected_driver("testdb").await;
    driver.execute("DROP TABLE IF EXISTS ddl_rename_src").await.unwrap();
    driver.execute("DROP TABLE IF EXISTS ddl_rename_dst").await.unwrap();
    driver.execute("CREATE TABLE ddl_rename_src (id INT PRIMARY KEY)").await.unwrap();

    driver.rename_table("testdb", "ddl_rename_src", "ddl_rename_dst").await.unwrap();

    // Old name gone
    let err = driver.execute("SELECT * FROM ddl_rename_src").await;
    assert!(err.is_err());

    // New name exists
    let result = driver.execute("SELECT COUNT(*) AS cnt FROM ddl_rename_dst").await.unwrap();
    assert_eq!(result.rows.len(), 1);

    driver.execute("DROP TABLE IF EXISTS ddl_rename_dst").await.unwrap();
}

#[tokio::test]
async fn drop_table() {
    let driver = helpers::connected_driver("testdb").await;
    driver.execute("CREATE TABLE IF NOT EXISTS ddl_drop (id INT PRIMARY KEY)").await.unwrap();

    driver.drop_table("testdb", "ddl_drop").await.unwrap();

    let err = driver.execute("SELECT * FROM ddl_drop").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn create_and_drop_view() {
    let driver = helpers::connected_driver("testdb").await;
    driver.execute("CREATE OR REPLACE VIEW ddl_test_view AS SELECT 1 AS x").await.unwrap();

    driver.drop_view("testdb", "ddl_test_view").await.unwrap();

    let err = driver.execute("SELECT * FROM ddl_test_view").await;
    assert!(err.is_err());
}

#[tokio::test]
async fn create_database() {
    let driver = helpers::connected_driver("testdb").await;
    let _ = driver.execute("DROP DATABASE IF EXISTS ddl_test_db").await;

    driver.create_database("ddl_test_db").await.unwrap();

    let dbs = driver.list_databases().await.unwrap();
    assert!(dbs.contains(&"ddl_test_db".to_string()));

    driver.execute("DROP DATABASE ddl_test_db").await.unwrap();
}

#[tokio::test]
async fn create_schema_creates_database() {
    let driver = helpers::connected_driver("testdb").await;
    let _ = driver.execute("DROP DATABASE IF EXISTS ddl_schema_test").await;

    // MySQL: create_schema = create_database
    driver.create_schema("testdb", "ddl_schema_test").await.unwrap();

    let dbs = driver.list_databases().await.unwrap();
    assert!(dbs.contains(&"ddl_schema_test".to_string()));

    driver.execute("DROP DATABASE ddl_schema_test").await.unwrap();
}
