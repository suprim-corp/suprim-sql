//! Types for the Structure Synchronization dialog.

use uuid::Uuid;

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
}

/// One side of the comparison (source or target).
#[derive(Default)]
pub(crate) struct Endpoint {
    pub conn_idx: usize,
    pub database: String,
    pub schema: String,
}

/// Result from dialog `show()` — tells the app what to do.
pub struct SyncDialogResult {
    /// `false` when the user closed the dialog.
    pub open: bool,
    /// Databases whose schemas need to be fetched (conn_id, database_name).
    pub schema_requests: Vec<(Uuid, String)>,
    /// Connections whose database lists need to be fetched.
    pub database_requests: Vec<Uuid>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub(crate) struct DiffEntry {
    pub label: String,
    pub kind: DiffKind,
    pub checked: bool,
    pub depth: u8,
}

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
pub(crate) enum DiffKind {
    Added,
    Removed,
    Modified,
}
