use eframe::egui::{self, CursorIcon};
use suprim_core::db::types::ViewNode;
use uuid::Uuid;

use crate::ui::icons;

/// Render the column list under a single view or materialized view node.
pub(super) fn render_view_detail(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    kind: &str,
    view: &ViewNode,
) {
    if view.columns.is_empty() {
        return;
    }

    let state_id = ui.make_persistent_id(format!(
        "{conn_id}:{db_name}:{schema_name}:{kind}:{}:cols",
        view.name
    ));
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), state_id, false);

    let (toggle_resp, header_resp, _body_resp) = state
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icons::db::column(icons::SIDEBAR_ICON, icons::db::COLOR_COLUMN));
                ui.label(format!("Columns ({})", view.columns.len()))
            })
        })
        .body(|ui| {
            for col in &view.columns {
                let null_marker = if col.nullable { "" } else { ", NOT NULL" };
                let display = format!("{} ({}){}", col.name, col.db_type, null_marker);
                ui.label(display);
            }
        });
    toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
    header_resp
        .inner
        .response
        .on_hover_cursor(CursorIcon::PointingHand);
}
