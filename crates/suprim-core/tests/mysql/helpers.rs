/// Shared test helpers for MySQL integration tests.
///
/// Requires running MySQL container:
///   docker run -d --name suprim-mysql -e MYSQL_ROOT_PASSWORD=testpass \
///     -e MYSQL_DATABASE=testdb -p 3307:3306 mysql:8.0 \
///     --default-authentication-plugin=mysql_native_password --performance-schema=ON
use suprim_core::db::connection::{ConnectionConfig, DriverParams};
use suprim_core::db::driver::DatabaseDriver;
use suprim_core::db::drivers::mysql::MysqlDriver;

pub fn test_config(database: &str) -> ConnectionConfig {
    let port: u16 = std::env::var("MYSQL_TEST_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3307);
    ConnectionConfig::new(
        "mysql-test",
        DriverParams::Mysql {
            host: "127.0.0.1".into(),
            port,
            database: database.into(),
            user: "root".into(),
            password_key: "testpass".into(),
        },
    )
}

/// Create a connected driver. Panics on failure.
pub async fn connected_driver(database: &str) -> MysqlDriver {
    let mut driver = MysqlDriver::new();
    driver.connect(&test_config(database)).await.unwrap();
    driver
}

/// Reset the `users` table to exactly 5 known rows.
/// Call this before any test that asserts exact row counts on `users`.
pub async fn reset_users_table(driver: &MysqlDriver) {
    driver
        .execute("DELETE FROM users WHERE email NOT IN ('alice@test.com','bob@test.com','charlie@test.com','diana@test.com','eve@test.com')")
        .await
        .unwrap();
}

/// Reset the `orders` table to exactly 5 known rows.
#[allow(dead_code)]
pub async fn reset_orders_table(driver: &MysqlDriver) {
    driver
        .execute("DELETE FROM orders WHERE id > 5")
        .await
        .unwrap();
}
