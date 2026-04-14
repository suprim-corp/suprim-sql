use crate::db::connection::DriverType;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Query error: {message}\nSQL: {sql}")]
    Query { sql: String, message: String },

    #[error("Schema error: {0}")]
    Schema(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Keychain error: {0}")]
    Keychain(String),

    #[error("{driver} driver error: {source}")]
    Driver {
        driver: DriverType,
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("Not connected")]
    NotConnected,

    #[error("SSH error: {0}")]
    Ssh(String),

    #[error("Operation cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, AppError>;

impl From<russh::Error> for AppError {
    fn from(e: russh::Error) -> Self {
        AppError::Ssh(e.to_string())
    }
}

impl AppError {
    pub fn driver(driver: DriverType, err: impl std::error::Error + Send + Sync + 'static) -> Self {
        AppError::Driver {
            driver,
            source: Box::new(err),
        }
    }

    pub fn connection(msg: impl Into<String>) -> Self {
        AppError::Connection(msg.into())
    }

    pub fn query(sql: impl Into<String>, msg: impl Into<String>) -> Self {
        AppError::Query {
            sql: sql.into(),
            message: msg.into(),
        }
    }

    pub fn config(msg: impl Into<String>) -> Self {
        AppError::Config(msg.into())
    }

    pub fn crypto(msg: impl Into<String>) -> Self {
        AppError::Crypto(msg.into())
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_connection() {
        let e = AppError::connection("timeout");
        assert_eq!(e.to_string(), "Connection error: timeout");
    }

    #[test]
    fn error_display_query() {
        let e = AppError::query("SELECT * FROM foo", "table not found");
        assert!(e.to_string().contains("table not found"));
        assert!(e.to_string().contains("SELECT * FROM foo"));
    }

    #[test]
    fn error_display_not_connected() {
        let e = AppError::NotConnected;
        assert_eq!(e.to_string(), "Not connected");
    }

    #[test]
    fn error_display_schema() {
        let e = AppError::Schema("no such table".to_string());
        assert!(e.to_string().contains("no such table"));
    }

    #[test]
    fn error_display_cancelled() {
        let e = AppError::Cancelled;
        assert_eq!(e.to_string(), "Operation cancelled");
    }

    #[test]
    fn error_display_crypto() {
        let e = AppError::crypto("bad key");
        assert!(e.to_string().contains("bad key"));
    }

    #[test]
    fn error_display_config() {
        let e = AppError::config("missing host");
        assert!(e.to_string().contains("missing host"));
    }

    #[test]
    fn error_display_keychain() {
        let e = AppError::Keychain("keychain locked".to_string());
        assert!(e.to_string().contains("keychain locked"));
    }

    #[test]
    fn error_driver_constructor() {
        use crate::db::connection::DriverType;
        use std::fmt;
        #[derive(Debug)]
        struct FakeErr;
        impl fmt::Display for FakeErr {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "fake")
            }
        }
        impl std::error::Error for FakeErr {}

        let e = AppError::driver(DriverType::Postgres, FakeErr);
        assert!(e.to_string().contains("PostgreSQL"));
        assert!(e.to_string().contains("fake"));
    }

    #[test]
    fn error_io_from() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let e = AppError::from(io_err);
        assert!(matches!(e, AppError::Io(_)));
        assert!(e.to_string().contains("file not found"));
    }

    #[test]
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AppError>();
    }
}
