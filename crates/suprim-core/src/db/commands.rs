//! Async channel protocol — `DbCommand` (UI→worker) and `DbEvent` (worker→UI).

use std::collections::HashMap;
use uuid::Uuid;

use super::{
    connection::ConnectionConfig,
    schema::{ExtensionInfo, ServerMetrics, SessionInfo, SlowQueryInfo},
    types::{QueryResult, SchemaNode},
    values::DbValue,
};

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
    /// Create a new database.
    CreateDatabase {
        conn_id: Uuid,
        name: String,
    },
    /// Create a new schema in a database.
    CreateSchema {
        conn_id: Uuid,
        database: String,
        name: String,
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
    /// Load active sessions and server metrics for the dashboard.
    LoadDashboard {
        conn_id: Uuid,
    },
    /// Kill (terminate) a session by PID.
    KillSession {
        conn_id: Uuid,
        pid: i32,
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
    /// A new database was created — triggers database list refresh.
    DatabaseCreated {
        conn_id: Uuid,
    },
    /// A new schema was created — triggers schema list refresh.
    SchemaCreated {
        conn_id: Uuid,
        database: String,
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
    /// Dashboard data loaded (sessions + metrics + slow queries).
    DashboardLoaded {
        conn_id: Uuid,
        sessions: Vec<SessionInfo>,
        metrics: ServerMetrics,
        slow_queries: Vec<SlowQueryInfo>,
    },
    /// A session was successfully killed.
    SessionKilled {
        conn_id: Uuid,
        pid: i32,
    },
}
