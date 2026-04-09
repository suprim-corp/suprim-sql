use crate::{
    db::{
        connection::ConnectionConfig,
        schema::ExtensionInfo,
        types::{QueryResult, SchemaNode},
    },
    error::Result,
};
use async_trait::async_trait;
use std::collections::HashMap;
use uuid::Uuid;

use super::{connection::DriverType, types::DbValue};

/// Core abstraction over any database engine.
/// All driver implementations must be `Send + Sync + Debug` for use across async tasks.
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
}

// ─── Async channel protocol ───────────────────────────────────────────────────

/// Commands sent from the UI thread to the DB worker.
#[derive(Debug)]
pub enum DbCommand {
    Connect {
        config: ConnectionConfig,
    },
    /// Test a connection without persisting it.
    TestConnection {
        config: ConnectionConfig,
    },
    Disconnect {
        conn_id: Uuid,
    },
    Execute {
        conn_id: Uuid,
        tab_id: Uuid,
        sql: String,
        /// Target database for query execution.
        /// When `Some`, the worker uses a database-specific pool.
        /// When `None`, the primary (default) pool is used.
        database: Option<String>,
    },
    /// List all databases on a connection.
    ListDatabases {
        conn_id: Uuid,
    },
    /// List schemas within a specific database.
    ListSchemas {
        conn_id: Uuid,
        database: String,
    },
    /// Load detail (tables/views/columns) for a single schema — lazy loading.
    LoadSchemaDetail {
        conn_id: Uuid,
        database: String,
        schema_name: String,
    },
    LoadTableData {
        conn_id: Uuid,
        tab_id: Uuid,
        database: Option<String>,
        schema: Option<String>,
        table: String,
        page: u32,
        page_size: u32,
        where_clause: Option<String>,
        order_clause: Option<String>,
    },
    InsertRow {
        conn_id: Uuid,
        tab_id: Uuid,
        table: String,
        values: HashMap<String, DbValue>,
    },
    UpdateRow {
        conn_id: Uuid,
        tab_id: Uuid,
        table: String,
        pk: HashMap<String, DbValue>,
        changes: HashMap<String, DbValue>,
    },
    DeleteRow {
        conn_id: Uuid,
        tab_id: Uuid,
        table: String,
        pk: HashMap<String, DbValue>,
    },
    /// Truncate a table (remove all rows).
    TruncateTable {
        conn_id: Uuid,
        database: String,
        schema_name: String,
        table_name: String,
    },
    /// Drop a table.
    DropTable {
        conn_id: Uuid,
        database: String,
        schema_name: String,
        table_name: String,
    },
    /// Drop a view.
    DropView {
        conn_id: Uuid,
        database: String,
        schema_name: String,
        view_name: String,
    },
    /// Rename a table.
    RenameTable {
        conn_id: Uuid,
        database: String,
        schema_name: String,
        old_name: String,
        new_name: String,
    },
    /// Load schema detail for two endpoints and return both for comparison.
    CompareSchemas {
        source_conn_id: Uuid,
        source_database: String,
        source_schema: String,
        target_conn_id: Uuid,
        target_database: String,
        target_schema: String,
    },
    /// Gracefully shut down the worker
    Shutdown,
}

/// Events sent from the DB worker back to the UI thread.
#[derive(Debug)]
pub enum DbEvent {
    Connected {
        conn_id: Uuid,
        /// Database names available on this server.
        databases: Vec<String>,
        /// Server version string (e.g. "PostgreSQL 16.2").
        server_version: Option<String>,
    },
    Disconnected {
        conn_id: Uuid,
    },
    QueryResult {
        tab_id: Uuid,
        result: QueryResult,
    },
    /// Response to ListDatabases.
    DatabasesListed {
        conn_id: Uuid,
        databases: Vec<String>,
    },
    /// Response to ListSchemas.
    SchemasListed {
        conn_id: Uuid,
        database: String,
        schemas: Vec<String>,
    },
    /// Detail loaded for a single schema (lazy loading).
    SchemaDetailLoaded {
        conn_id: Uuid,
        database: String,
        schema_name: String,
        schema_node: SchemaNode,
    },
    RowMutated {
        tab_id: Uuid,
        rows_affected: u64,
    },
    /// DDL operation completed — triggers a schema refresh on the UI side.
    DdlCompleted {
        conn_id: Uuid,
        database: String,
        schema_name: String,
    },
    Error {
        /// `None` for connection-level errors
        tab_id: Option<Uuid>,
        conn_id: Option<Uuid>,
        message: String,
    },
    /// Result of a test connection attempt.
    TestConnectionResult {
        success: bool,
        message: String,
    },
    /// Both schemas loaded for comparison.
    SchemasCompared {
        source: SchemaNode,
        target: SchemaNode,
        source_extensions: Vec<ExtensionInfo>,
        target_extensions: Vec<ExtensionInfo>,
    },
}
