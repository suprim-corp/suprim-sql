//! Step 1: Select source and target endpoints (connection + database + schema).
//!
//! - `endpoint_picker.rs` — connection/database/schema combo boxes + summary
//! - `info_panel.rs` — two-column metadata grid

mod endpoint_picker;
mod info_panel;

pub(crate) use endpoint_picker::render_endpoint_pickers;
pub(crate) use endpoint_picker::render_endpoint_summary;
pub(crate) use info_panel::render_information_panels;
