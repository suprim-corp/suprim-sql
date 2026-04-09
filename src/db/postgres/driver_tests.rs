//! Unit tests for `PostgresDriver`.

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::db::connection::{ConnectionConfig, DriverParams};
    use crate::db::driver::DatabaseDriver;
    use crate::error::AppError;

    use super::super::PostgresDriver;

    #[test]
    fn new_driver_not_connected() {
        let driver = PostgresDriver::new();
        assert!(!driver.is_connected());
    }

    #[test]
    fn default_driver_not_connected() {
        let driver = PostgresDriver::default();
        assert!(!driver.is_connected());
    }

    #[test]
    fn driver_type_returns_postgres() {
        use crate::db::connection::DriverType;
        let driver = PostgresDriver::new();
        assert_eq!(driver.driver_type(), DriverType::Postgres);
    }

    #[tokio::test]
    async fn disconnect_without_connect_is_ok() {
        let mut driver = PostgresDriver::new();
        assert!(driver.disconnect().await.is_ok());
    }

    #[tokio::test]
    async fn ping_without_connect_returns_not_connected() {
        let driver = PostgresDriver::new();
        let err = driver.ping().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_without_connect_returns_not_connected() {
        let driver = PostgresDriver::new();
        let err = driver.execute("SELECT 1").await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_with_params_without_connect_returns_not_connected() {
        let driver = PostgresDriver::new();
        let err = driver
            .execute_with_params("SELECT $1", vec![])
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn list_databases_without_connect_returns_not_connected() {
        let driver = PostgresDriver::new();
        let err = driver.list_databases().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn table_data_without_connect_returns_not_connected() {
        let driver = PostgresDriver::new();
        let err = driver
            .table_data(None, None, "users", 0, 50, None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn insert_row_without_connect_returns_not_connected() {
        let driver = PostgresDriver::new();
        let err = driver
            .insert_row("users", HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn update_row_without_connect_returns_not_connected() {
        let driver = PostgresDriver::new();
        let err = driver
            .update_row("users", HashMap::new(), HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn delete_row_without_connect_returns_not_connected() {
        let driver = PostgresDriver::new();
        let err = driver
            .delete_row("users", HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn connect_wrong_driver_params_returns_error() {
        let mut driver = PostgresDriver::new();
        let config = ConnectionConfig::new(
            "bad",
            DriverParams::Sqlite {
                path: "/tmp/test.db".into(),
            },
        );
        let err = driver.connect(&config).await.unwrap_err();
        assert!(matches!(err, AppError::Connection(_)));
    }
}
