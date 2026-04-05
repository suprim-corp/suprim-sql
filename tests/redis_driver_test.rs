//! Integration tests for the Redis driver.
//! Requires Docker — uses testcontainers-rs to spin up redis:7.

#[cfg(test)]
mod tests {
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::redis::Redis;

    async fn redis_config() -> (suprim_sql::db::ConnectionConfig, impl Drop) {
        let container = Redis::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(6379).await.unwrap();
        let config = suprim_sql::db::ConnectionConfig::new(
            "test-redis",
            suprim_sql::db::DriverParams::Redis {
                host: "127.0.0.1".into(),
                port,
                db_index: 0,
                password_key: None,
            },
        );
        (config, container)
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_connect_ok() {
        let (_config, _container) = redis_config().await;
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_set_get() {
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_scan_keys() {
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_pubsub() {
        todo!()
    }
}
