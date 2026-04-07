use crate::{
    db::{
        connection::ConnectionConfig,
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
}

// ─── Async channel protocol ───────────────────────────────────────────────────

/// Commands sent from the UI thread to the DB worker.
#[derive(Debug)]
pub enum DbCommand {
    Connect {
        config: ConnectionConfig,
    },
    Disconnect {
        conn_id: Uuid,
    },
    Execute {
        conn_id: Uuid,
        tab_id: Uuid,
        sql: String,
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
    Error {
        /// `None` for connection-level errors
        tab_id: Option<Uuid>,
        conn_id: Option<Uuid>,
        message: String,
    },
}
