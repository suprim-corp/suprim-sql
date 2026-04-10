use eframe::egui::{self, CursorIcon};
use suprim_sql::db::types::SchemaNode;
use uuid::Uuid;

use super::table_context_menu;
use super::table_detail_renderer;
use super::SidebarAction;

pub(super) fn render_tables_folder(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    schema_node: &SchemaNode,
    action: &mut Option<SidebarAction>,
) {
    let label = format!(
        "{} Tables ({})",
        egui_phosphor::regular::TABLE,
        schema_node.tables.len()
    );
    let tables_resp = egui::CollapsingHeader::new(label)
        .id_salt(format!("{conn_id}:{db_name}:{schema_name}:tables"))
        .show(ui, |ui| {
            for table in &schema_node.tables {
                let tbl_name = &table.name;
                let tbl_label = match table.row_count {
                    Some(c) => format!(
                        "{} {} (~{})",
                        egui_phosphor::regular::TABLE,
                        tbl_name,
                        format_row_count(c)
                    ),
                    None => format!("{} {}", egui_phosphor::regular::TABLE, tbl_name),
                };

                let state_id = ui.make_persistent_id(format!(
                    "{conn_id}:{db_name}:{schema_name}:tbl:{tbl_name}"
                ));
                let state = egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    state_id,
                    false,
                );

                let (toggle_resp, header_resp, _body_resp) = state
                    .show_header(ui, |ui| {
                        let resp = ui
                            .selectable_label(false, &tbl_label)
                            .on_hover_cursor(CursorIcon::PointingHand);
                        resp
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

                toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
                let label_resp = &header_resp.inner;

                // Right-click context menu on the label
                table_context_menu::render_table_context_menu(
                    label_resp,
                    conn_id,
                    db_name,
                    schema_name,
                    table,
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
    tables_resp.header_response.context_menu(|ui| {
        if ui
            .button(format!(
                "{}  New Table...",
                egui_phosphor::regular::PLUS_CIRCLE
            ))
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::NewTable {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
            });
            ui.close();
        }
    });
    tables_resp
        .header_response
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
