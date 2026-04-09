/// Structure Synchronization dialog — compare schemas between two connections.
///
/// File layout:
/// - `state.rs`  — dialog struct, construction, event-driven data updates
/// - `dialog.rs` — UI orchestration (show method, egui::Window, step routing)
/// - `types.rs`  — shared types (ConnInfo, Endpoint, DiffEntry, WizardStep, etc.)
/// - `steps/`    — wizard step implementations:
///     - `select/`  — endpoint picker, header banner, info panel
///     - `compare/` — schema comparison logic
///     - `review/`  — diff review with checkboxes
///     - `preview/` — DDL script viewer
///     - `execute/` — DDL execution with progress
mod dialog;
pub(crate) mod state;
pub(crate) mod steps;
pub(crate) mod types;

// Re-export primary entry points for external callers.
pub use state::StructureSyncDialog;
pub use types::{ConnInfo, ConnMeta, DbInfo};
