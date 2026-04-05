//! Integration tests for the MySQL driver.
//! Requires Docker — uses testcontainers-rs to spin up mysql:8.

#[cfg(test)]
mod tests {
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::mysql::Mysql;

    async fn mysql_config() -> (suprim_sql::db::ConnectionConfig, impl Drop) {
        let container = Mysql::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(3306).await.unwrap();
        let config = suprim_sql::db::ConnectionConfig::new(
            "test-mysql",
            suprim_sql::db::DriverParams::Mysql {
                host: "127.0.0.1".into(),
                port,
                database: "mysql".into(),
                user: "root".into(),
                password_key: "test-password-key".into(),
            },
        );
        (config, container)
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_connect_ok() {
        let (_config, _container) = mysql_config().await;
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
