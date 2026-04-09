// src/db/postgres/mod.rs
//
// PostgreSQL driver — split into submodules:
//   connection_url  — URL building + percent-encoding
//   type_mapping    — pg_value_from_row + rows_to_query_result
//   schema_loader   — load_schema (lazy) + load_schema_detail
//   queries         — execute, execute_with_params, table_data, insert/update/delete

mod connection_url;
mod function_loader;
mod queries;
mod schema_loader;
mod type_mapping;

pub use connection_url::{build_connection_url, urlencoding_simple};
pub use type_mapping::{pg_value_from_row, rows_to_query_result};

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use crate::db::connection::{ConnectionConfig, DriverParams, DriverType};
use crate::db::driver::DatabaseDriver;
use crate::db::types::{DbValue, QueryResult, SchemaNode};
use crate::error::{AppError, Result};

// ─── Driver ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct PostgresDriver {
    /// Primary pool (for the database specified in connection config).
    pool: Option<PgPool>,
    /// Per-database pools for cross-database browsing (behind Mutex for interior mutability).
    db_pools: Mutex<HashMap<String, PgPool>>,
    /// Connection options needed to create per-database pools.
    connect_opts: Option<PgConnectOptions>,
}

impl PostgresDriver {
    pub fn new() -> Self {
        Self {
            pool: None,
            db_pools: Mutex::new(HashMap::new()),
            connect_opts: None,
        }
    }

    fn pool(&self) -> Result<&PgPool> {
        self.pool.as_ref().ok_or(AppError::NotConnected)
    }

    /// Get or create a pool for a specific database.
    async fn pool_for_db(&self, database: &str) -> Result<PgPool> {
        // Check if pool already exists.
        {
            let pools = self.db_pools.lock().unwrap();
            if let Some(pool) = pools.get(database) {
                return Ok(pool.clone());
            }
        }
        // Create new pool for this database.
        let base_opts = self
            .connect_opts
            .as_ref()
            .ok_or(AppError::NotConnected)?
            .clone();
        let opts = base_opts.database(database);
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect_with(opts)
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;
        {
            let mut pools = self.db_pools.lock().unwrap();
            pools.insert(database.to_string(), pool.clone());
        }
        Ok(pool)
    }
}

impl Default for PostgresDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()> {
        let (host, port, database, user, password) = match &config.params {
            DriverParams::Postgres {
                host,
                port,
                database,
                user,
                password_key,
            } => {
                // In production, retrieve password from keychain using password_key.
                // For now, treat password_key as the actual password for testability.
                (
                    host.as_str(),
                    *port,
                    database.as_str(),
                    user.as_str(),
                    password_key.as_str(),
                )
            }
            _ => return Err(AppError::connection("PostgresDriver requires Postgres params")),
        };

        let opts = PgConnectOptions::new()
            .host(host)
            .port(port)
            .database(database)
            .username(user)
            .password(password);

        // Store base connection options (without database) for cross-db pool creation.
        self.connect_opts = Some(
            PgConnectOptions::new()
                .host(host)
                .port(port)
                .username(user)
                .password(password),
        );

        let pool = PgPoolOptions::new()
            .max_connections(10)
            .min_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect_with(opts)
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;

        self.pool = Some(pool);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }
        let pools: Vec<PgPool> = {
            let mut locked = self.db_pools.lock().unwrap();
            locked.drain().map(|(_, p)| p).collect()
        };
        for pool in pools {
            pool.close().await;
        }
        self.connect_opts = None;
        Ok(())
    }

    async fn ping(&self) -> Result<()> {
        let pool = self.pool()?;
        sqlx::query("SELECT 1")
            .execute(pool)
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;
        Ok(())
    }

    async fn execute(&self, sql: &str) -> Result<QueryResult> {
        queries::execute(self.pool()?, sql).await
    }

    async fn execute_on_database(&self, sql: &str, database: &str) -> Result<QueryResult> {
        let pool = self.pool_for_db(database).await?;
        queries::execute(&pool, sql).await
    }

    async fn execute_with_params(&self, sql: &str, params: Vec<DbValue>) -> Result<QueryResult> {
        queries::execute_with_params(self.pool()?, sql, params).await
    }

    async fn list_databases(&self) -> Result<Vec<String>> {
        schema_loader::list_databases(self.pool()?).await
    }

    async fn list_schemas(&self, database: &str) -> Result<Vec<String>> {
        let pool = self.pool_for_db(database).await?;
        schema_loader::list_schemas(&pool).await
    }

    async fn load_schema_detail(&self, database: &str, schema_name: &str) -> Result<SchemaNode> {
        let pool = self.pool_for_db(database).await?;
        schema_loader::load_schema_detail(&pool, schema_name).await
    }

    async fn table_data(
        &self,
        database: Option<&str>,
        schema: Option<&str>,
        table: &str,
        page: u32,
        page_size: u32,
        where_clause: Option<&str>,
        order_clause: Option<&str>,
    ) -> Result<QueryResult> {
        let pool = match database {
            Some(db) => self.pool_for_db(db).await?,
            None => self.pool()?.clone(),
        };
        queries::table_data(&pool, schema, table, page, page_size, where_clause, order_clause).await
    }

    async fn insert_row(&self, table: &str, values: HashMap<String, DbValue>) -> Result<u64> {
        queries::insert_row(self.pool()?, table, values).await
    }

    async fn update_row(
        &self,
        table: &str,
        pk: HashMap<String, DbValue>,
        changes: HashMap<String, DbValue>,
    ) -> Result<u64> {
        queries::update_row(self.pool()?, table, pk, changes).await
    }

    async fn delete_row(&self, table: &str, pk: HashMap<String, DbValue>) -> Result<u64> {
        queries::delete_row(self.pool()?, table, pk).await
    }

    fn driver_type(&self) -> DriverType {
        DriverType::Postgres
    }

    fn is_connected(&self) -> bool {
        self.pool.is_some()
    }
}

// ─── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::DriverParams;

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
