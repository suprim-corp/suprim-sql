use crate::{
    db::{
        connection::ConnectionConfig,
        schema::{ExtensionInfo, ServerMetrics, SessionInfo, SlowQueryInfo},
        types::{QueryResult, SchemaNode},
    },
    error::Result,
};
use async_trait::async_trait;
use std::collections::HashMap;

use super::{connection::DriverType, types::DbValue};

/// Core abstraction over any database engine.
/// All driver implementations must be `Send + Sync + Debug` for use across async tasks.
#[allow(clippy::too_many_arguments)]
#[async_trait]
pub trait DatabaseDriver: Send + Sync + std::fmt::Debug {
    /// Establish connection using the provided config.
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()>;

    /// Close the connection and release resources.
    async fn disconnect(&mut self) -> Result<()>;

    /// Check if the connection is alive (lightweight ping).
    async fn ping(&self) -> Result<()>;

    // ── Query ────────────────────────────────────────────────────────────────

    /// Execute a raw SQL string and return results.
    async fn execute(&self, sql: &str) -> Result<QueryResult>;

    /// Execute SQL on a specific database (cross-database query support).
    /// Default implementation ignores the database parameter and delegates to `execute()`.
    /// Drivers that support cross-database pools (e.g. PostgreSQL) should override this.
    async fn execute_on_database(&self, sql: &str, _database: &str) -> Result<QueryResult> {
        self.execute(sql).await
    }

    /// Execute SQL with positional parameters.
    async fn execute_with_params(&self, sql: &str, params: Vec<DbValue>) -> Result<QueryResult>;

    // ── Schema (lazy 3-level hierarchy) ──────────────────────────────────────

    /// List all accessible databases on this server.
    async fn list_databases(&self) -> Result<Vec<String>>;

    /// List schemas within a specific database.
    /// For drivers that need a separate connection per database (e.g. PostgreSQL),
    /// this may only work for the currently connected database.
    async fn list_schemas(&self, database: &str) -> Result<Vec<String>>;

    /// Load full detail for a single named schema: tables, views, columns, indexes, FKs.
    async fn load_schema_detail(&self, database: &str, schema_name: &str) -> Result<SchemaNode>;

    /// List installed extensions for a database (database-level objects).
    /// Default returns empty — override for drivers that support extensions.
    async fn list_extensions(&self, _database: &str) -> Result<Vec<ExtensionInfo>> {
        Ok(Vec::new())
    }

    /// Fetch a page of rows from a table, with optional WHERE/ORDER BY clauses.
    async fn table_data(
        &self,
        database: Option<&str>,
        schema: Option<&str>,
        table: &str,
        page: u32,
        page_size: u32,
        where_clause: Option<&str>,
        order_clause: Option<&str>,
    ) -> Result<QueryResult>;

    // ── Mutations (inline table editor) ──────────────────────────────────────

    /// Insert a new row. Returns number of rows affected.
    async fn insert_row(
        &self,
        table: &str,
        values: HashMap<String, DbValue>,
    ) -> Result<u64>;

    /// Update an existing row identified by primary key values.
    async fn update_row(
        &self,
        table: &str,
        pk: HashMap<String, DbValue>,
        changes: HashMap<String, DbValue>,
    ) -> Result<u64>;

    /// Delete a row identified by primary key values.
    async fn delete_row(
        &self,
        table: &str,
        pk: HashMap<String, DbValue>,
    ) -> Result<u64>;

    // ── Metadata ─────────────────────────────────────────────────────────────

    fn driver_type(&self) -> DriverType;

    fn is_connected(&self) -> bool;

    // ── DDL operations ───────────────────────────────────────────────────────
    // Default implementations generate standard SQL and delegate to execute().
    // Drivers can override for dialect-specific behavior.

    /// Truncate a table (remove all rows without dropping the table).
    async fn truncate_table(&self, schema: &str, table: &str) -> Result<()> {
        let sql = format!("TRUNCATE TABLE \"{}\".\"{}\"", schema, table);
        self.execute(&sql).await?;
        Ok(())
    }

    /// Drop a table.
    async fn drop_table(&self, schema: &str, table: &str) -> Result<()> {
        let sql = format!("DROP TABLE \"{}\".\"{}\"", schema, table);
        self.execute(&sql).await?;
        Ok(())
    }

    /// Drop a view.
    async fn drop_view(&self, schema: &str, view: &str) -> Result<()> {
        let sql = format!("DROP VIEW IF EXISTS \"{}\".\"{}\"", schema, view);
        self.execute(&sql).await?;
        Ok(())
    }

    /// Rename a table.
    async fn rename_table(&self, schema: &str, old_name: &str, new_name: &str) -> Result<()> {
        let sql = format!(
            "ALTER TABLE \"{}\".\"{}\" RENAME TO \"{}\"",
            schema, old_name, new_name
        );
        self.execute(&sql).await?;
        Ok(())
    }

    /// Create a new database.
    async fn create_database(&self, name: &str) -> Result<()> {
        let sql = format!("CREATE DATABASE \"{}\"", name);
        self.execute(&sql).await?;
        Ok(())
    }

    /// Create a new schema in a specific database.
    async fn create_schema(&self, database: &str, name: &str) -> Result<()> {
        let sql = format!("CREATE SCHEMA \"{}\"", name);
        self.execute_on_database(&sql, database).await?;
        Ok(())
    }

    // ── Server Dashboard ─────────────────────────────────────────────────

    /// List active sessions on the server.
    /// Default returns empty — override for drivers that support session introspection.
    async fn list_sessions(&self) -> Result<Vec<SessionInfo>> {
        Ok(Vec::new())
    }

    /// Get server-level metrics (connections, uptime, etc.).
    /// Default returns empty metrics — override for drivers with status queries.
    async fn server_metrics(&self) -> Result<ServerMetrics> {
        Ok(ServerMetrics::default())
    }

    /// Terminate a session/process by PID.
    /// Default is a no-op — override for drivers that support session termination.
    async fn kill_session(&self, _pid: i32) -> Result<()> {
        Ok(())
    }

    /// List slow queries from server statistics (e.g. pg_stat_statements).
    /// Default returns empty — override for drivers that support query statistics.
    async fn list_slow_queries(&self) -> Result<Vec<SlowQueryInfo>> {
        Ok(Vec::new())
    }
}
