pub(crate) mod about_dialog;
mod clipboard_formatters;
mod connection_dialog;
mod connection_dialog_config;
#[cfg(target_os = "macos")]
pub(crate) mod custom_title_bar;
mod editor_themes;
#[cfg(target_os = "macos")]
pub mod macos_menu;
mod result_grid;
mod sidebar;
mod sql_autocomplete;
mod sql_editor_tab;
mod sql_highlighter;
mod statusbar;
mod tab_bar;
mod tab_manager;
mod table_editor_tab;
mod table_viewer_tab;

pub use connection_dialog::{ConnectionDialog, DialogResult};
pub use sidebar::{Sidebar, SidebarAction};
pub use statusbar::StatusBar;
pub use tab_manager::TabManager;
