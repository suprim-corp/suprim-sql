//! Shared types for Structure Synchronization — the cross-crate interface.
//!
//! These types live in `suprim-core` so both `suprim-app` (binary) and
//! `suprim-extensions` (private crate) can reference them without circular deps.
//! The actual UI implementation lives in `suprim-extensions`.

use uuid::Uuid;

use crate::db::schema::{ExtensionInfo, SchemaNode};

// ── Connection descriptors ──────────────────────────────────────────────────

/// Database entry with its schemas.
#[derive(Clone)]
pub struct DbInfo {
    pub name: String,
    pub schemas: Vec<String>,
}

/// Connection metadata displayed in the Information panel.
#[derive(Clone, Default)]
pub struct ConnMeta {
    pub driver_type: String,
    pub host: String,
    pub port: String,
    pub server_version: Option<String>,
}

/// Lightweight connection descriptor passed in from the outside.
#[derive(Clone)]
pub struct ConnInfo {
    pub conn_id: Uuid,
    pub label: String,
    pub databases: Vec<DbInfo>,
    pub meta: ConnMeta,
    /// Whether this connection is currently active (connected to the server).
    pub connected: bool,
}

// ── Dialog request / result types ───────────────────────────────────────────

/// Request to kick off a schema comparison (sent via SyncDialogResult).
pub struct CompareRequest {
    pub source_conn_id: Uuid,
    pub source_database: String,
    pub source_schema: String,
    pub target_conn_id: Uuid,
    pub target_database: String,
    pub target_schema: String,
}

/// Result from dialog `show()` — tells the app what to do.
pub struct SyncDialogResult {
    /// `false` when the user closed the dialog.
    pub open: bool,
    /// Databases whose schemas need to be fetched (conn_id, database_name).
    pub schema_requests: Vec<(Uuid, String)>,
    /// Connections whose database lists need to be fetched.
    pub database_requests: Vec<Uuid>,
    /// Connections that need to be connected first.
    pub connect_requests: Vec<Uuid>,
    /// Schema comparison request (triggered by "Compare" button).
    pub compare_request: Option<CompareRequest>,
}

// ── Trait object interface for tool dialogs ─────────────────────────────────

/// Trait for tool dialogs that the premium crate can provide and the
/// app crate can display via `dyn ToolDialog`.
///
/// This keeps the app crate free of any Structure Sync implementation code.
pub trait ToolDialog: Send {
    /// Render one frame of the dialog. Returns the result for the app to act on.
    fn show(&mut self, ctx: &egui::Context) -> SyncDialogResult;

    /// Update database list for a connection when new data arrives.
    fn update_databases(&mut self, conn_id: Uuid, databases: Vec<String>);

    /// Update schema list for a connection+database when new data arrives.
    fn update_schemas(&mut self, conn_id: Uuid, database: &str, schemas: Vec<String>);

    /// Update server version for a connection when Connected event arrives.
    fn update_server_version(&mut self, conn_id: Uuid, version: Option<String>);

    /// Called when the DB worker returns both schema nodes + extensions.
    fn on_schemas_compared(
        &mut self,
        source: SchemaNode,
        target: SchemaNode,
        source_extensions: Vec<ExtensionInfo>,
        target_extensions: Vec<ExtensionInfo>,
    );
}
