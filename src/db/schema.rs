/// Schema tree model — sidebar data structure for database/schema/table browsing.
/// Separated from value types (`values.rs`) since these serve different concerns.

/// Full schema tree — root of the sidebar model
#[derive(Debug, Clone, Default)]
pub struct SchemaTree {
    pub databases: Vec<DatabaseNode>,
}

#[derive(Debug, Clone)]
pub struct DatabaseNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub schemas: Vec<SchemaNode>,
}

#[derive(Debug, Clone)]
pub struct SchemaNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub tables: Vec<TableNode>,
    pub views: Vec<ViewNode>,
    pub materialized_views: Vec<ViewNode>,
    pub sequences: Vec<SequenceNode>,
    pub functions: Vec<FunctionNode>,
    /// Whether table/view detail has been loaded (for lazy loading in UI).
    pub loaded: bool,
}

#[derive(Debug, Clone)]
pub struct TableNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub columns: Vec<ColumnNode>,
    pub indexes: Vec<IndexNode>,
    pub foreign_keys: Vec<ForeignKeyNode>,
    pub row_count: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ViewNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub columns: Vec<ColumnNode>,
}

#[derive(Debug, Clone)]
pub struct ColumnNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub db_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct IndexNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
}

#[derive(Debug, Clone)]
pub struct SequenceNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub data_type: String,
    pub start_value: i64,
    pub increment: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub last_value: Option<i64>,
    /// Table.column that owns this sequence (e.g. "users.id"), if any.
    pub owner: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ForeignKeyNode {
    pub id: uuid::Uuid,
    pub name: String,
    pub columns: Vec<String>,
    pub ref_table: String,
    pub ref_columns: Vec<String>,
}

/// A database function or stored procedure.
#[derive(Debug, Clone)]
pub struct FunctionNode {
    pub id: uuid::Uuid,
    /// Function name (without arguments — use `identity_args` for overload distinction).
    pub name: String,
    /// Argument types as a comma-separated string (used to distinguish overloads).
    pub identity_args: String,
    /// Full function signature: `name(arg_types)`.
    pub signature: String,
    /// Return type (e.g. "void", "integer", "SETOF record").
    pub return_type: String,
    /// Language (e.g. "plpgsql", "sql", "c").
    pub language: String,
    /// Full source definition (`CREATE OR REPLACE FUNCTION …`).
    pub definition: String,
    /// Whether this is a procedure (CALL) rather than a function (SELECT).
    pub is_procedure: bool,
}

/// A database extension (database-level object, not schema-level).
#[derive(Debug, Clone)]
pub struct ExtensionInfo {
    /// Extension name (e.g. "pgvector", "postgis").
    pub name: String,
    /// Installed version (e.g. "0.7.0").
    pub version: String,
}

// ─── Server Dashboard types ──────────────────────────────────────────────────

/// An active session/process on the database server.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    /// Process ID.
    pub pid: i32,
    /// User running the session.
    pub user: String,
    /// Database the session is connected to.
    pub database: String,
    /// Current state (e.g. "active", "idle", "idle in transaction").
    pub state: String,
    /// Duration of the current query/state as a human-readable string.
    pub duration: String,
    /// Current query text (may be empty).
    pub query: String,
}

/// Server-level metrics for the dashboard.
#[derive(Debug, Clone, Default)]
pub struct ServerMetrics {
    /// Total number of connected sessions/threads.
    pub connected_sessions: i64,
    /// Number of currently active (running) queries.
    pub active_queries: i64,
    /// Server uptime as a human-readable string.
    pub uptime: String,
    /// Total number of transactions committed.
    pub total_transactions: i64,
    /// Number of slow queries (from pg_stat_statements or similar).
    pub slow_queries: i64,
    /// Maximum allowed connections.
    pub max_connections: i64,
    /// Total bytes received as a human-readable string.
    pub bytes_received: String,
    /// Total bytes sent as a human-readable string.
    pub bytes_sent: String,
}

/// A slow query entry from pg_stat_statements.
#[derive(Debug, Clone)]
pub struct SlowQueryInfo {
    /// Normalized query text.
    pub query: String,
    /// Number of times the query was called.
    pub calls: i64,
    /// Total execution time in milliseconds.
    pub total_time_ms: f64,
    /// Mean execution time in milliseconds.
    pub mean_time_ms: f64,
    /// Maximum execution time in milliseconds.
    pub max_time_ms: f64,
    /// Total number of rows returned.
    pub rows: i64,
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_tree_default_is_empty() {
        let tree = SchemaTree::default();
        assert!(tree.databases.is_empty());
    }
}
