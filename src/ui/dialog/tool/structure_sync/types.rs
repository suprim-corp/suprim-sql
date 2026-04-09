//! Types for the Structure Synchronization dialog.

use uuid::Uuid;

/// Wizard step in the Structure Synchronization flow.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum WizardStep {
    /// Step 1: Select source and target endpoints.
    #[default]
    Select,
    /// Step 2: Compare schemas (async fetch + diff).
    Compare,
    /// Step 3: Review diff entries with checkboxes.
    Review,
    /// Step 4: Preview generated DDL script.
    Preview,
    /// Step 5: Execute DDL against target.
    Execute,
}

/// State of the comparison operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CompareState {
    /// Not yet compared.
    #[default]
    Idle,
    /// Waiting for DB worker to return both schemas.
    Loading,
    /// Comparison finished, diff entries populated.
    Done,
}

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

/// One side of the comparison (source or target).
#[derive(Default)]
pub(crate) struct Endpoint {
    pub conn_idx: usize,
    pub database: String,
    pub schema: String,
}

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

#[derive(Clone)]
pub(crate) struct DiffEntry {
    pub label: String,
    pub kind: DiffKind,
    pub checked: bool,
    pub depth: u8,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum DiffKind {
    Added,
    Removed,
    Modified,
}
