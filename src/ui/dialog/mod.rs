/// Dialog modules — modal UI windows (About, Connection, Structure Sync, etc.).
pub(crate) mod about_dialog;
mod connection_dialog;
mod connection_dialog_config;
pub(crate) mod structure_sync_dialog;
mod structure_sync_renderer;
pub(crate) mod structure_sync_types;

pub use connection_dialog::{ConnectionDialog, DialogResult};
