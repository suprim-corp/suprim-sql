//! `DatabaseDriver` trait implementation for `MysqlDriver`.
//!
//! MySQL uses backtick quoting (`` `table` ``) and `database = schema` semantics.
//! DDL methods are overridden to use MySQL-specific SQL dialect.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::db::connection::{ConnectionConfig, DriverParams, DriverType, SslMode};
use crate::db::driver::DatabaseDriver;
use crate::db::schema::{ServerMetrics, SessionInfo, SlowQueryInfo};
use crate::db::types::{DbValue, QueryResult, SchemaNode};
use crate::error::{AppError, Result};
use crate::storage::credential;

use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};

use super::MysqlDriver;

#[async_trait]
impl DatabaseDriver for MysqlDriver {
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()> {
        let (host, port, database, user, password) = match &config.params {
            DriverParams::Mysql {
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
            _ => return Err(AppError::connection("MysqlDriver requires Mysql params")),
        };

        let mut opts = MySqlConnectOptions::new()
            .host(host)
            .port(port)
            .database(database)
            .username(user)
            .password(&password);

        // Apply SSL mode
        opts = apply_mysql_ssl(opts, &config.tls);

        let pool_opts = MySqlPoolOptions::new()
            .max_connections(10)
            .min_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(10));

        // Connect — with fallback for Prefer mode.
        // MySQL 5.7 uses TLS 1.0/1.1 which rustls doesn't support.
        // When SslMode::Prefer, try TLS first, fallback to plaintext on TLS/IO errors.
        let pool = match pool_opts.connect_with(opts.clone()).await {
            Ok(p) => p,
            Err(ref e) if config.tls.ssl_mode == SslMode::Prefer && is_tls_error(e) => {
                let plain_opts = opts.ssl_mode(sqlx::mysql::MySqlSslMode::Disabled);
                MySqlPoolOptions::new()
                    .max_connections(10)
                    .min_connections(1)
                    .acquire_timeout(std::time::Duration::from_secs(10))
                    .connect_with(plain_opts)
                    .await
                    .map_err(|e2| AppError::connection(e2.to_string()))?
            }
            Err(e) => return Err(AppError::connection(e.to_string())),
        };

        self.pool = Some(pool);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }
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

    // ── Query ────────────────────────────────────────────────────────────────

    async fn execute(&self, sql: &str) -> Result<QueryResult> {
        super::queries::execute(self.pool()?, sql).await
    }

    async fn execute_on_database(&self, sql: &str, database: &str) -> Result<QueryResult> {
        super::queries::execute_on_database(self.pool()?, sql, database).await
    }

    async fn execute_with_params(&self, sql: &str, params: Vec<DbValue>) -> Result<QueryResult> {
        super::queries::execute_with_params(self.pool()?, sql, params).await
    }

    // ── Schema ───────────────────────────────────────────────────────────────

    async fn list_databases(&self) -> Result<Vec<String>> {
        super::schema_loader::list_databases(self.pool()?).await
    }

    /// MySQL databases ARE schemas — return the database name itself.
    async fn list_schemas(&self, database: &str) -> Result<Vec<String>> {
        let _pool = self.pool()?;
        super::schema_loader::list_schemas(database).await
    }

    async fn load_schema_detail(&self, database: &str, _schema_name: &str) -> Result<SchemaNode> {
        // In MySQL, schema_name == database name. We use `database` for the query.
        super::schema_loader::load_schema_detail(self.pool()?, database).await
    }

    async fn table_data(
        &self,
        database: Option<&str>,
        _schema: Option<&str>,
        table: &str,
        page: u32,
        page_size: u32,
        where_clause: Option<&str>,
        order_clause: Option<&str>,
    ) -> Result<QueryResult> {
        super::queries::table_data(
            self.pool()?,
            database,
            table,
            page,
            page_size,
            where_clause,
            order_clause,
        )
        .await
    }

    // ── Mutations ────────────────────────────────────────────────────────────

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

    // ── Metadata ─────────────────────────────────────────────────────────────

    fn driver_type(&self) -> DriverType {
        DriverType::Mysql
    }

    fn is_connected(&self) -> bool {
        self.pool.is_some()
    }

    // ── DDL operations (MySQL backtick quoting) ──────────────────────────────

    async fn truncate_table(&self, _schema: &str, table: &str) -> Result<()> {
        let sql = format!("TRUNCATE TABLE {}", super::quote_ident(table));
        self.execute(&sql).await?;
        Ok(())
    }

    async fn drop_table(&self, _schema: &str, table: &str) -> Result<()> {
        let sql = format!("DROP TABLE {}", super::quote_ident(table));
        self.execute(&sql).await?;
        Ok(())
    }

    async fn drop_view(&self, _schema: &str, view: &str) -> Result<()> {
        let sql = format!("DROP VIEW IF EXISTS {}", super::quote_ident(view));
        self.execute(&sql).await?;
        Ok(())
    }

    async fn rename_table(&self, _schema: &str, old_name: &str, new_name: &str) -> Result<()> {
        let sql = format!(
            "RENAME TABLE {} TO {}",
            super::quote_ident(old_name),
            super::quote_ident(new_name)
        );
        self.execute(&sql).await?;
        Ok(())
    }

    async fn create_database(&self, name: &str) -> Result<()> {
        let sql = format!("CREATE DATABASE {}", super::quote_ident(name));
        self.execute(&sql).await?;
        Ok(())
    }

    /// MySQL schema = database, so create_schema delegates to create_database.
    async fn create_schema(&self, _database: &str, name: &str) -> Result<()> {
        self.create_database(name).await
    }

    // ── Server Dashboard ─────────────────────────────────────────────────────

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

/// Map `SslMode` to `MySqlConnectOptions` ssl settings.
fn apply_mysql_ssl(
    mut opts: MySqlConnectOptions,
    tls: &crate::db::connection::TlsConfig,
) -> MySqlConnectOptions {
    opts = match tls.ssl_mode {
        SslMode::Disable => opts.ssl_mode(sqlx::mysql::MySqlSslMode::Disabled),
        SslMode::Prefer => opts.ssl_mode(sqlx::mysql::MySqlSslMode::Preferred),
        SslMode::Require => opts.ssl_mode(sqlx::mysql::MySqlSslMode::Required),
        SslMode::VerifyCa | SslMode::VerifyFull => opts.ssl_mode(sqlx::mysql::MySqlSslMode::VerifyCa),
    };

    // Cert paths only meaningful for Require, VerifyCa, and VerifyFull
    if matches!(tls.ssl_mode, SslMode::Require | SslMode::VerifyCa | SslMode::VerifyFull) {
        if let Some(ca_path) = &tls.ca_cert_path {
            opts = opts.ssl_ca(ca_path);
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

/// Check if an sqlx error is TLS-related (handshake failure, protocol mismatch, etc.).
/// Used by the Prefer mode fallback — avoids fragile string matching on error messages.
fn is_tls_error(err: &sqlx::Error) -> bool {
    matches!(err, sqlx::Error::Tls(_) | sqlx::Error::Io(_))
}
