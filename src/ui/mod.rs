mod clipboard_formatters;
#[cfg(target_os = "macos")]
pub(crate) mod custom_title_bar;
pub(crate) mod dialog;
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

pub(crate) use dialog::about_dialog;
pub use dialog::{ConnectionDialog, DialogResult};
pub use sidebar::{Sidebar, SidebarAction};
pub use statusbar::StatusBar;
pub use tab_manager::TabManager;
