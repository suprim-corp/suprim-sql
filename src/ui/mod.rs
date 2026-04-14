#[cfg(target_os = "macos")]
pub(crate) mod custom_title_bar;
pub(crate) mod dialog;
#[cfg(target_os = "macos")]
pub mod macos_menu;
pub(crate) mod query_history;
mod server_dashboard;
pub(crate) mod shared;
mod sidebar;
pub(crate) mod sql_editor;
mod statusbar;
mod tab_bar;
mod tab_manager;
mod tab_opener;
mod tab_snapshot;
mod table_editor_tab;
pub(crate) mod table_viewer_tab;

pub(crate) use dialog::about_dialog;
pub use dialog::{
    ConnectionDialog, DeleteConnectionDialog, DeleteConnectionResult, DialogResult, InputDialog,
    InputDialogKind, InputDialogResult,
};
pub use sidebar::{Sidebar, SidebarAction};
pub use statusbar::StatusBar;
pub use tab_manager::TabManager;
