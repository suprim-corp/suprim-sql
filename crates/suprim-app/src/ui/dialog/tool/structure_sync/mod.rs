/// Structure Synchronization dialog — compare schemas between two connections.
///
/// File layout:
/// - `state.rs`                 — dialog struct, construction, event-driven data updates
/// - `dialog.rs`                — UI orchestration (show method, egui::Window, step routing)
/// - `diff_results_renderer.rs` — diff results UI (loading, groups, entries, inline rows)
/// - `bottom_bar.rs`            — bottom bar (Options, Copy Script, Close, Compare)
/// - `types.rs`                 — shared types (ConnInfo, Endpoint, DiffEntry, WizardStep, etc.)
/// - `steps/`                   — wizard step implementations:
///     - `select/`  — endpoint picker, info panel
///     - `compare/` — schema comparison logic + DDL generation
mod bottom_bar;
mod dialog;
mod diff_results_renderer;
pub(crate) mod state;
pub(crate) mod steps;
pub(crate) mod types;

// Re-export primary entry points for external callers.
pub use state::StructureSyncDialog;
pub use types::{ConnInfo, ConnMeta, DbInfo};
