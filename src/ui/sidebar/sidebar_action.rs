use suprim_sql::db::types::TableNode;
use uuid::Uuid;

/// Action the sidebar wants the app to perform.
#[allow(dead_code)]
pub enum SidebarAction {
    /// User clicked a disconnected connection — initiate connect.
    Connect {
        conn_id: Uuid,
    },
    NewConnection,
    EditConnection {
        conn_id: Uuid,
    },
    OpenSqlTab {
        conn_id: Uuid,
        /// Active database context for the SQL tab.
        database: Option<String>,
        /// All databases available on this connection.
        databases: Vec<String>,
    },
    OpenTableViewer {
        conn_id: Uuid,
        database: String,
        schema_name: String,
        table_name: String,
    },
    /// Open the table structure editor tab.
    EditTable {
        conn_id: Uuid,
        database: String,
        schema_name: String,
        table: TableNode,
    },
    Disconnect {
        conn_id: Uuid,
    },
    /// User wants to delete a connection from config entirely.
    DeleteConnection {
        conn_id: Uuid,
        conn_name: String,
    },
    LoadSchemaDetail {
        conn_id: Uuid,
        database: String,
        schema_name: String,
    },
    ListSchemas {
        conn_id: Uuid,
        database: String,
    },
    UpdateVisibleDatabases {
        conn_id: Uuid,
        visible: Option<Vec<String>>,
    },
    /// Reload the schema detail for a specific schema (Refresh).
    RefreshSchema {
        conn_id: Uuid,
        database: String,
        schema_name: String,
    },
    /// Execute TRUNCATE TABLE on the given table.
    TruncateTable {
        conn_id: Uuid,
        database: String,
        schema_name: String,
        table_name: String,
    },
    /// Execute DROP TABLE on the given table.
    DropTable {
        conn_id: Uuid,
        database: String,
        schema_name: String,
        table_name: String,
    },
    /// Execute DROP VIEW on the given view.
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
}
