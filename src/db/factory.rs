use crate::db::connection::{ConnectionConfig, DriverType};
use crate::db::driver::DatabaseDriver;
use crate::db::mongodb_driver::MongoDriver;
use crate::db::mssql::MssqlDriver;
use crate::db::mysql::MysqlDriver;
use crate::db::postgres::PostgresDriver;
use crate::db::redis_driver::RedisDriver;
use crate::db::sqlite::SqliteDriver;
use crate::error::Result;

/// Instantiate the correct driver from a ConnectionConfig at runtime.
pub struct DbFactory;

impl DbFactory {
    pub fn create(config: &ConnectionConfig) -> Result<Box<dyn DatabaseDriver>> {
        match config.driver_type() {
            DriverType::Postgres => Ok(Box::new(PostgresDriver::new())),
            DriverType::Sqlite => Ok(Box::new(SqliteDriver::new())),
            DriverType::Mysql => Ok(Box::new(MysqlDriver::new())),
            DriverType::Redis => Ok(Box::new(RedisDriver::new())),
            DriverType::MongoDB => Ok(Box::new(MongoDriver::new())),
            DriverType::Mssql => Ok(Box::new(MssqlDriver::new())),
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
    fn create_sqlite_driver_ok() {
        let config = ConnectionConfig::new(
            "test",
            DriverParams::Sqlite {
                path: "/tmp/test.db".into(),
            },
        );
        let driver = DbFactory::create(&config);
        assert!(driver.is_ok());
        assert_eq!(driver.unwrap().driver_type(), DriverType::Sqlite);
    }

    #[test]
    fn create_mysql_driver_ok() {
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
        let driver = DbFactory::create(&config);
        assert!(driver.is_ok());
        assert_eq!(driver.unwrap().driver_type(), DriverType::Mysql);
    }

    #[test]
    fn create_redis_driver_ok() {
        let config = ConnectionConfig::new(
            "test",
            DriverParams::Redis {
                host: "localhost".into(),
                port: 6379,
                db_index: 0,
                password_key: None,
            },
        );
        let driver = DbFactory::create(&config);
        assert!(driver.is_ok());
        assert_eq!(driver.unwrap().driver_type(), DriverType::Redis);
    }

    #[test]
    fn create_mongodb_driver_ok() {
        let config = ConnectionConfig::new(
            "test",
            DriverParams::MongoDB {
                uri: "mongodb://localhost:27017/testdb".into(),
                password_key: None,
            },
        );
        let driver = DbFactory::create(&config);
        assert!(driver.is_ok());
        assert_eq!(driver.unwrap().driver_type(), DriverType::MongoDB);
    }

    #[test]
    fn create_mssql_driver_ok() {
        let config = ConnectionConfig::new(
            "test",
            DriverParams::Mssql {
                host: "localhost".into(),
                port: 1433,
                database: "db".into(),
                user: "sa".into(),
                password_key: "key".into(),
            },
        );
        let driver = DbFactory::create(&config);
        assert!(driver.is_ok());
        assert_eq!(driver.unwrap().driver_type(), DriverType::Mssql);
    }
}
