use crate::db::connection::{ConnectionConfig, DriverType};
use crate::db::driver::DatabaseDriver;
use crate::db::drivers::postgres::PostgresDriver;
use crate::error::{AppError, Result};
use crate::premium::PremiumGate;

/// Instantiate the correct driver from a ConnectionConfig at runtime.
pub struct DbFactory;

impl DbFactory {
    pub fn create(
        config: &ConnectionConfig,
        gate: &dyn PremiumGate,
    ) -> Result<Box<dyn DatabaseDriver>> {
        // License gate: check if driver type is allowed on current tier.
        if let Err(msg) = gate.can_use_driver(&config.driver_type()) {
            return Err(AppError::connection(msg));
        }

        // Try premium drivers first (MongoDB, MSSQL — only available with premium feature).
        if let Some(result) = gate.create_driver(config) {
            return result;
        }

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
    use crate::premium::FreeTierGate;

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
        let gate = FreeTierGate;
        let driver = DbFactory::create(&config, &gate);
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
        let gate = FreeTierGate;
        assert!(DbFactory::create(&config, &gate).is_err());
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
        let gate = FreeTierGate;
        assert!(DbFactory::create(&config, &gate).is_err());
    }

    #[test]
    fn create_mongodb_blocked_on_free() {
        let config = ConnectionConfig::new(
            "test",
            DriverParams::MongoDB {
                uri: "mongodb://localhost".into(),
                password_key: None,
            },
        );
        let gate = FreeTierGate;
        let result = DbFactory::create(&config, &gate);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Premium"));
    }
}
