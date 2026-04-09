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

// ── Diff model ──────────────────────────────────────────────────────────────

/// What kind of change an entry represents.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiffKind {
    /// Object exists in source but not target → needs to be created on target.
    Added,
    /// Object exists in target but not source → needs to be removed from target.
    Removed,
    /// Object exists on both sides but differs.
    Modified,
}

/// What type of database object an entry represents.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObjectType {
    Table,
    Column,
    Index,
    ForeignKey,
    View,
    MaterializedView,
    Sequence,
    Function,
}

impl ObjectType {
    /// Icon string for this object type (using egui_phosphor icons).
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Table => egui_phosphor::regular::TABLE,
            Self::Column => egui_phosphor::regular::COLUMNS,
            Self::Index => egui_phosphor::regular::LIST_NUMBERS,
            Self::ForeignKey => egui_phosphor::regular::LINK,
            Self::View => egui_phosphor::regular::EYE,
            Self::MaterializedView => egui_phosphor::regular::DATABASE,
            Self::Sequence => egui_phosphor::regular::HASH,
            Self::Function => egui_phosphor::regular::FUNCTION,
        }
    }
}

/// A single diff entry — one database object that differs between source and target.
#[derive(Clone)]
pub(crate) struct DiffEntry {
    /// The object type (Table, Column, Index, etc.).
    pub object_type: ObjectType,
    /// The object name.
    pub name: String,
    /// Optional detail string (e.g. column type, index columns).
    pub detail: String,
    /// What kind of change.
    pub kind: DiffKind,
    /// Whether the user wants to include this in the DDL script.
    pub checked: bool,
    /// Child entries (e.g. columns, indexes, FKs under a table).
    pub children: Vec<DiffEntry>,
}

/// A top-level group in the diff results UI (Modified / Created / Deleted).
pub(crate) struct DiffGroup {
    pub kind: DiffKind,
    pub entries: Vec<DiffEntry>,
}

impl DiffGroup {
    pub fn label(&self) -> &'static str {
        match self.kind {
            DiffKind::Modified => "Objects to be modified",
            DiffKind::Added => "Objects to be created",
            DiffKind::Removed => "Objects to be dropped",
        }
    }

    /// Count of checked entries (top-level only).
    pub fn checked_count(&self) -> usize {
        self.entries.iter().filter(|e| e.checked).count()
    }

    /// Total number of entries (top-level only).
    pub fn total_count(&self) -> usize {
        self.entries.len()
    }
}
