use eframe::egui::{self, CursorIcon};
use suprim_core::db::types::SchemaNode;
use uuid::Uuid;

use crate::ui::icons;

pub(super) fn render_sequences_folder(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    schema_node: &SchemaNode,
) {
    let state_id = ui.make_persistent_id(format!("{conn_id}:{db_name}:{schema_name}:sequences"));
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), state_id, false);

    let (toggle_resp, header_resp, _body_resp) = state
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icons::db::sequence(
                    icons::SIDEBAR_ICON,
                    icons::db::COLOR_SEQUENCE,
                ));
                ui.label(format!("Sequences ({})", schema_node.sequences.len()))
            })
        })
        .body(|ui| {
            for seq in &schema_node.sequences {
                ui.label(&seq.name);
            }
        });
    toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
    header_resp
        .inner
        .response
        .on_hover_cursor(CursorIcon::PointingHand);
}
