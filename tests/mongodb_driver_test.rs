//! Integration tests for the MongoDB driver.
//! Requires Docker — uses testcontainers-rs to spin up mongo:7.

#[cfg(test)]
mod tests {
    use testcontainers::runners::AsyncRunner;
    use testcontainers_modules::mongo::Mongo;

    async fn mongo_config() -> (suprim_sql::db::ConnectionConfig, impl Drop) {
        let container = Mongo::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(27017).await.unwrap();
        let config = suprim_sql::db::ConnectionConfig::new(
            "test-mongo",
            suprim_sql::db::DriverParams::MongoDB {
                uri: format!("mongodb://127.0.0.1:{}", port),
                password_key: None,
            },
        );
        (config, container)
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_connect_ok() {
        let (_config, _container) = mongo_config().await;
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_list_collections() {
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_find_documents() {
        todo!()
    }

    #[tokio::test]
    #[ignore = "requires Docker + driver not yet implemented"]
    async fn test_insert_document() {
        todo!()
    }
}
