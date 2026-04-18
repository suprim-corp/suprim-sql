use eframe::egui::{self, CursorIcon};
use suprim_core::db::types::SchemaNode;
use uuid::Uuid;

use super::table_context_menu;
use super::table_detail_renderer;
use super::SidebarAction;
use crate::ui::icons;

pub(super) fn render_tables_folder(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    schema_node: &SchemaNode,
    action: &mut Option<SidebarAction>,
) {
    let folder_state_id =
        ui.make_persistent_id(format!("{conn_id}:{db_name}:{schema_name}:tables"));
    let folder_state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        folder_state_id,
        false,
    );

    let (toggle_resp, header_resp, _body_resp) = folder_state
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icons::db::table(
                    icons::SIDEBAR_ICON,
                    icons::db::COLOR_TABLE,
                ));
                ui.add(
                    egui::Label::new(format!("Tables ({})", schema_node.tables.len()))
                        .sense(egui::Sense::click()),
                )
            })
        })
        .body(|ui| {
            for table in &schema_node.tables {
                let tbl_name = &table.name;
                let tbl_suffix = match table.row_count {
                    Some(c) => format!(" (~{})", format_row_count(c)),
                    None => String::new(),
                };

                let state_id = ui.make_persistent_id(format!(
                    "{conn_id}:{db_name}:{schema_name}:tbl:{tbl_name}"
                ));
                let state = egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    state_id,
                    false,
                );

                let (tbl_toggle, tbl_header, _tbl_body) = state
                    .show_header(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(icons::db::table(
                                icons::SIDEBAR_ICON,
                                icons::db::COLOR_TABLE,
                            ));
                            ui.selectable_label(false, format!("{}{}", tbl_name, tbl_suffix))
                                .on_hover_cursor(CursorIcon::PointingHand)
                        })
                        .inner
                    })
                    .body(|ui| {
                        table_detail_renderer::render_table_detail(
                            ui,
                            conn_id,
                            db_name,
                            schema_name,
                            table,
                        );
                    });

                tbl_toggle.on_hover_cursor(CursorIcon::PointingHand);
                let label_resp = &tbl_header.inner;

                // Right-click context menu on the label
                let func_sigs: Vec<String> = schema_node
                    .functions
                    .iter()
                    .map(|f| f.signature.clone())
                    .collect();
                table_context_menu::render_table_context_menu(
                    label_resp,
                    conn_id,
                    db_name,
                    schema_name,
                    table,
                    &func_sigs,
                    &schema_node.tables,
                    &schema_node.views,
                    action,
                );

                // Click table name → open data viewer tab
                if label_resp.clicked() || label_resp.double_clicked() {
                    *action = Some(SidebarAction::OpenTableViewer {
                        conn_id,
                        database: db_name.to_owned(),
                        schema_name: schema_name.to_owned(),
                        table_name: tbl_name.clone(),
                    });
                }
            }
        });
    // Right-click on "Tables" folder header → "New Table..."
    header_resp.inner.response.context_menu(|ui| {
        if ui
            .button(format!("{}  New Table...", icons::ph::plus_circle()))
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::NewTable {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
                schema_functions: schema_node
                    .functions
                    .iter()
                    .map(|f| f.signature.clone())
                    .collect(),
            });
            ui.close();
        }
    });
    // Click "Tables" folder label → toggle expand/collapse. Must run before
    // `on_hover_cursor` (which consumes the response).
    super::sidebar_renderer::toggle_on_label_click(
        ui.ctx(),
        folder_state_id,
        &header_resp.inner.inner,
    );

    toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
    header_resp
        .inner
        .inner
        .on_hover_cursor(CursorIcon::PointingHand);
}

fn format_row_count(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}
