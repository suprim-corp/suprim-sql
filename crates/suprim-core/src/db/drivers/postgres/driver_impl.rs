//! `DatabaseDriver` trait implementation for `PostgresDriver`.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::db::connection::{ConnectionConfig, DriverParams, DriverType, SslMode};
use crate::db::driver::DatabaseDriver;
use crate::db::schema::{ExtensionInfo, ServerMetrics, SessionInfo, SlowQueryInfo};
use crate::db::types::{DbValue, QueryResult, SchemaNode};
use crate::error::{AppError, Result};
use crate::storage::credential;

use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};

use super::PostgresDriver;

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
                let decrypted = credential::decrypt(password_key);
                (
                    host.as_str(),
                    *port,
                    database.as_str(),
                    user.as_str(),
                    decrypted,
                )
            }
            _ => return Err(AppError::connection("PostgresDriver requires Postgres params")),
        };

        let mut opts = PgConnectOptions::new()
            .host(host)
            .port(port)
            .database(database)
            .username(user)
            .password(&password);

        // Apply SSL mode
        opts = apply_pg_ssl(opts, &config.tls);

        // Store base connection options (without database) for cross-db pool creation.
        let mut base_opts = PgConnectOptions::new()
            .host(host)
            .port(port)
            .username(user)
            .password(&password);

        base_opts = apply_pg_ssl(base_opts, &config.tls);
        self.connect_opts = Some(base_opts);

        let pool = PgPoolOptions::new()
            .max_connections(10)
            .min_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .idle_timeout(std::time::Duration::from_secs(300))
            .test_before_acquire(true)
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
        let pools: Vec<sqlx::postgres::PgPool> = {
            let mut locked = self.db_pools.lock().unwrap_or_else(|e| e.into_inner());
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
        super::queries::execute(self.pool()?, sql).await
    }

    async fn execute_on_database(&self, sql: &str, database: &str) -> Result<QueryResult> {
        let pool = self.pool_for_db(database).await?;
        super::queries::execute(&pool, sql).await
    }

    async fn execute_with_params(&self, sql: &str, params: Vec<DbValue>) -> Result<QueryResult> {
        super::queries::execute_with_params(self.pool()?, sql, params).await
    }

    async fn list_databases(&self) -> Result<Vec<String>> {
        super::schema_loader::list_databases(self.pool()?).await
    }

    async fn list_schemas(&self, database: &str) -> Result<Vec<String>> {
        let pool = self.pool_for_db(database).await?;
        super::schema_loader::list_schemas(&pool).await
    }

    async fn load_schema_detail(&self, database: &str, schema_name: &str) -> Result<SchemaNode> {
        let pool = self.pool_for_db(database).await?;
        super::schema_loader::load_schema_detail(&pool, schema_name).await
    }

    async fn list_extensions(&self, database: &str) -> Result<Vec<ExtensionInfo>> {
        let pool = self.pool_for_db(database).await?;
        Ok(super::extension_loader::load_extensions(&pool).await)
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
        super::queries::table_data(&pool, schema, table, page, page_size, where_clause, order_clause).await
    }

    async fn insert_row(&self, table: &str, values: HashMap<String, DbValue>) -> Result<u64> {
        super::queries::insert_row(self.pool()?, table, values).await
    }

    async fn update_row(
        &self,
        table: &str,
        pk: HashMap<String, DbValue>,
        changes: HashMap<String, DbValue>,
    ) -> Result<u64> {
        super::queries::update_row(self.pool()?, table, pk, changes).await
    }

    async fn delete_row(&self, table: &str, pk: HashMap<String, DbValue>) -> Result<u64> {
        super::queries::delete_row(self.pool()?, table, pk).await
    }

    fn driver_type(&self) -> DriverType {
        DriverType::Postgres
    }

    fn is_connected(&self) -> bool {
        self.pool.is_some()
    }

    async fn list_sessions(&self) -> Result<Vec<SessionInfo>> {
        Ok(super::dashboard_loader::load_sessions(self.pool()?).await)
    }

    async fn server_metrics(&self) -> Result<ServerMetrics> {
        Ok(super::dashboard_loader::load_metrics(self.pool()?).await)
    }

    async fn kill_session(&self, pid: i32) -> Result<()> {
        super::dashboard_loader::kill_session(self.pool()?, pid).await
    }

    async fn list_slow_queries(&self) -> Result<Vec<SlowQueryInfo>> {
        Ok(super::dashboard_loader::load_slow_queries(self.pool()?).await)
    }
}

/// Map `SslMode` to `PgConnectOptions` ssl settings.
fn apply_pg_ssl(
    mut opts: PgConnectOptions,
    tls: &crate::db::connection::TlsConfig,
) -> PgConnectOptions {
    opts = match tls.ssl_mode {
        SslMode::Disable => opts.ssl_mode(PgSslMode::Disable),
        SslMode::Prefer => opts.ssl_mode(PgSslMode::Prefer),
        SslMode::Require => opts.ssl_mode(PgSslMode::Require),
        SslMode::VerifyCa => opts.ssl_mode(PgSslMode::VerifyCa),
        SslMode::VerifyFull => opts.ssl_mode(PgSslMode::VerifyFull),
    };

    // Cert paths only meaningful for Require, VerifyCa, and VerifyFull
    if matches!(tls.ssl_mode, SslMode::Require | SslMode::VerifyCa | SslMode::VerifyFull) {
        if let Some(ca_path) = &tls.ca_cert_path {
            opts = opts.ssl_root_cert(ca_path);
        }
        if let Some(cert_path) = &tls.client_cert_path {
            opts = opts.ssl_client_cert(cert_path);
        }
        if let Some(key_path) = &tls.client_key_path {
            opts = opts.ssl_client_key(key_path);
        }
    }

    opts
}
