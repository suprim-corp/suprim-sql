use crate::db::connection::{ConnectionConfig, DriverType};
use crate::db::driver::DatabaseDriver;
use crate::db::postgres::PostgresDriver;
use crate::error::{AppError, Result};

/// Instantiate the correct driver from a ConnectionConfig at runtime.
pub struct DbFactory;

impl DbFactory {
    pub fn create(config: &ConnectionConfig) -> Result<Box<dyn DatabaseDriver>> {
        match config.driver_type() {
            DriverType::Postgres => Ok(Box::new(PostgresDriver::new())),
            other => Err(AppError::connection(format!(
                "{} driver not yet available — coming soon",
                other
            ))),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::{ConnectionConfig, DriverParams};

    #[test]
    fn create_postgres_driver_ok() {
        let config = ConnectionConfig::new(
            "test",
            DriverParams::Postgres {
                host: "localhost".into(),
                port: 5432,
                database: "db".into(),
                user: "user".into(),
                password_key: "key".into(),
            },
        );
        let driver = DbFactory::create(&config);
        assert!(driver.is_ok());
        assert_eq!(driver.unwrap().driver_type(), DriverType::Postgres);
    }

    #[test]
    fn create_sqlite_returns_not_available() {
        let config = ConnectionConfig::new(
            "test",
            DriverParams::Sqlite {
                path: "/tmp/test.db".into(),
            },
        );
        assert!(DbFactory::create(&config).is_err());
    }

    #[test]
    fn create_mysql_returns_not_available() {
        let config = ConnectionConfig::new(
            "test",
            DriverParams::Mysql {
                host: "localhost".into(),
                port: 3306,
                database: "db".into(),
                user: "user".into(),
                password_key: "key".into(),
            },
        );
        assert!(DbFactory::create(&config).is_err());
    }
}
