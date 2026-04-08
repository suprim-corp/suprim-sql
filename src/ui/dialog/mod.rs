/// Dialog modules — modal UI windows (About, Connection, etc.).
pub(crate) mod about_dialog;
mod connection_dialog;
mod connection_dialog_config;

pub use connection_dialog::{ConnectionDialog, DialogResult};
