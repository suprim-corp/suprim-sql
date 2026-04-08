use eframe::egui::{self, CursorIcon};
use suprim_sql::db::types::{SchemaNode, ViewNode};
use uuid::Uuid;

use super::table_context_menu;
use super::table_detail_renderer;
use super::view_detail_renderer;
use super::SidebarAction;

// ─── Tables ─────────────────────────────────────────────────────────────────

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
    egui::CollapsingHeader::new(label)
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
}

// ─── Views & Materialized Views (shared logic) ─────────────────────────────

pub(super) fn render_views_folder(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    views: &[ViewNode],
    action: &mut Option<SidebarAction>,
) {
    render_view_like_folder(
        ui,
        conn_id,
        db_name,
        schema_name,
        views,
        action,
        "Views",
        "view",
        egui_phosphor::regular::EYE,
        "Open View",
    );
}

pub(super) fn render_materialized_views_folder(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    views: &[ViewNode],
    action: &mut Option<SidebarAction>,
) {
    render_view_like_folder(
        ui,
        conn_id,
        db_name,
        schema_name,
        views,
        action,
        "Materialized Views",
        "matview",
        egui_phosphor::regular::SQUARES_FOUR,
        "Open Materialized View",
    );
}

/// Shared renderer for Views and Materialized Views folders.
fn render_view_like_folder(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    views: &[ViewNode],
    action: &mut Option<SidebarAction>,
    folder_name: &str,
    kind: &str,
    icon: &str,
    open_label: &str,
) {
    let label = format!("{} {} ({})", icon, folder_name, views.len());
    egui::CollapsingHeader::new(label)
        .id_salt(format!("{conn_id}:{db_name}:{schema_name}:{kind}s"))
        .show(ui, |ui| {
            for view in views {
                let v_name = &view.name;
                let v_label = format!("{} {}", icon, v_name);

                let header_resp = egui::CollapsingHeader::new(&v_label)
                    .id_salt(format!("{conn_id}:{db_name}:{schema_name}:{kind}:{v_name}"))
                    .show(ui, |ui| {
                        view_detail_renderer::render_view_detail(
                            ui,
                            conn_id,
                            db_name,
                            schema_name,
                            kind,
                            view,
                        );
                    })
                    .header_response;

                table_context_menu::render_view_context_menu(
                    &header_resp,
                    conn_id,
                    db_name,
                    schema_name,
                    v_name,
                    open_label,
                    action,
                );
            }
        });
}

// ─── Sequences ──────────────────────────────────────────────────────────────

pub(super) fn render_sequences_folder(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    schema_node: &SchemaNode,
) {
    let label = format!(
        "{} Sequences ({})",
        egui_phosphor::regular::LIST_NUMBERS,
        schema_node.sequences.len()
    );
    egui::CollapsingHeader::new(label)
        .id_salt(format!("{conn_id}:{db_name}:{schema_name}:sequences"))
        .show(ui, |ui| {
            for seq in &schema_node.sequences {
                ui.label(&seq.name);
            }
        });
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn format_row_count(count: u64) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}
