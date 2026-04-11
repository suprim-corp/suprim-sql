/// Dialog modules — modal UI windows (About, Connection, Delete Connection, Structure Sync, etc.).
pub(crate) mod about_dialog;
mod connection_dialog;
mod connection_dialog_config;
pub(crate) mod delete_connection;
pub(crate) mod input_dialog;
pub(crate) mod tool;

pub use connection_dialog::{ConnectionDialog, DialogResult};
pub use delete_connection::{DeleteConnectionDialog, DeleteConnectionResult};
pub use input_dialog::{InputDialog, InputDialogKind, InputDialogResult};
