use eframe::egui::{self, CursorIcon};
use suprim_core::db::types::SchemaNode;
use uuid::Uuid;

use super::SidebarAction;
use crate::ui::icons;

pub(super) fn render_sequences_folder(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    schema_node: &SchemaNode,
    action: &mut Option<SidebarAction>,
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
                ui.selectable_label(false, format!("Sequences ({})", schema_node.sequences.len()))
            })
        })
        .body(|ui| {
            if schema_node.sequences.is_empty() {
                ui.weak("No sequences.");
                return;
            }
            for seq in &schema_node.sequences {
                let resp = ui
                    .selectable_label(false, &seq.name)
                    .on_hover_cursor(CursorIcon::Default);
                resp.context_menu(|ui| {
                    if ui
                        .button("Copy name")
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        ui.ctx().copy_text(seq.name.clone());
                        ui.close();
                    }
                });
            }
        });
    // Right-click on "Sequences" folder header → "Refresh"
    header_resp.inner.response.context_menu(|ui| {
        if ui
            .button(format!("{}  Refresh", icons::ph::arrows_clockwise()))
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::RefreshSchema {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
            });
            ui.close();
        }
    });

    // Click "Sequences" folder label → toggle expand/collapse. Must run before
    // `on_hover_cursor` (which consumes the response).
    super::sidebar_renderer::toggle_on_label_click(
        ui.ctx(),
        state_id,
        &header_resp.inner.inner,
    );

    toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
    header_resp
        .inner
        .inner
        .on_hover_cursor(CursorIcon::PointingHand);
}
