//! Step 1: Select source and target endpoints (connection + database + schema).
//!
//! - `header_banner.rs` — summary banner with arrow + bottom bar
//! - `endpoint_picker.rs` — connection/database/schema combo boxes
//! - `info_panel.rs` — two-column metadata grid

mod endpoint_picker;
mod header_banner;
mod info_panel;

pub(crate) use endpoint_picker::render_endpoint_pickers;
pub(crate) use header_banner::{render_bottom_bar, render_header_banner};
pub(crate) use info_panel::render_information_panels;
