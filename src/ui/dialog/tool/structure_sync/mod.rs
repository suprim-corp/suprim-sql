/// Structure Synchronization dialog — compare schemas between two connections.
///
/// File layout:
/// - `state.rs`           — dialog struct, construction, event-driven data updates
/// - `dialog.rs`          — UI orchestration (show method, egui::Window)
/// - `comparison.rs`      — schema comparison logic and DDL generation
/// - `header_banner.rs`   — header banner, bottom bar, re-exports
/// - `endpoint_picker.rs` — source/target connection+database+schema pickers
/// - `info_panel.rs`      — two-column information grid
/// - `types.rs`           — shared types (ConnInfo, Endpoint, DiffEntry, etc.)
mod comparison;
mod dialog;
mod endpoint_picker;
pub(crate) mod header_banner;
mod info_panel;
pub(crate) mod state;
pub(crate) mod types;

// Re-export primary entry points for external callers.
pub use state::StructureSyncDialog;
pub use types::{ConnInfo, ConnMeta, DbInfo};
