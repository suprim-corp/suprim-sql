#[cfg(target_os = "macos")]
pub(crate) mod custom_title_bar;
pub(crate) mod dialog;
#[cfg(target_os = "macos")]
pub mod macos_menu;
mod server_dashboard;
pub(crate) mod shared;
mod sidebar;
pub(crate) mod sql_editor;
mod statusbar;
mod tab_bar;
mod tab_manager;
mod table_editor_tab;
mod table_viewer_tab;

pub(crate) use dialog::about_dialog;
pub use dialog::{
    ConnectionDialog, DeleteConnectionDialog, DeleteConnectionResult, DialogResult, InputDialog,
    InputDialogKind, InputDialogResult,
};
pub use sidebar::{Sidebar, SidebarAction};
pub use statusbar::StatusBar;
pub use tab_manager::TabManager;
