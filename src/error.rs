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

    #[error("Operation cancelled")]
    Cancelled,
}

pub type Result<T> = std::result::Result<T, AppError>;

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
    fn error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<AppError>();
    }
}
