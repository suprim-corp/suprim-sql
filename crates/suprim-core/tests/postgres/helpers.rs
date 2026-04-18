/// Shared test helpers for PostgreSQL integration tests.
///
/// Requires running PostgreSQL container:
///   docker run -d --name suprim-postgres -e POSTGRES_PASSWORD=testpass \
///     -e POSTGRES_DB=testdb -p 5433:5432 postgres:15
use suprim_core::db::connection::{ConnectionConfig, DriverParams};
use suprim_core::db::driver::DatabaseDriver;
use suprim_core::db::drivers::postgres::PostgresDriver;

pub fn test_config(database: &str) -> ConnectionConfig {
    let port: u16 = std::env::var("PG_TEST_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5433);
    ConnectionConfig::new(
        "pg-test",
        DriverParams::Postgres {
            host: "127.0.0.1".into(),
            port,
            database: database.into(),
            user: "postgres".into(),
            password_key: "testpass".into(),
        },
    )
}

/// Create a connected driver. Panics on failure.
pub async fn connected_driver(database: &str) -> PostgresDriver {
    let mut driver = PostgresDriver::new();
    driver.connect(&test_config(database)).await.unwrap();
    driver
}

/// Reset the `users` table to exactly 5 known rows.
pub async fn reset_users_table(driver: &PostgresDriver) {
    driver
        .execute("DELETE FROM users WHERE email NOT IN ('alice@test.com','bob@test.com','charlie@test.com','diana@test.com','eve@test.com')")
        .await
        .unwrap();
}
