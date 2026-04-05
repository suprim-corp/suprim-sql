//! Integration tests for the PostgreSQL driver.
//! Requires Docker — uses testcontainers-rs to spin up postgres:15.

#[cfg(test)]
mod tests {
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::postgres::Postgres;

    async fn pg_config() -> (suprim_sql::db::ConnectionConfig, impl Drop) {
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let config = suprim_sql::db::ConnectionConfig::new(
            "test-postgres",
            suprim_sql::db::DriverParams::Postgres {
                host: "127.0.0.1".into(),
                port,
                database: "postgres".into(),
                user: "postgres".into(),
                password_key: "test-password-key".into(),
            },
        );
        (config, container)
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_connect_ok() {
        let (_config, _container) = pg_config().await;
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_connect_wrong_password() {
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_execute_select() {
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_execute_insert_update_delete() {
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_load_schema() {
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_table_data_pagination() {
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_transaction_rollback() {
        todo!()
    }
}
