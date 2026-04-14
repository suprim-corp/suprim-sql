use eframe::egui::{self, CursorIcon};
use suprim_sql::db::types::ViewNode;
use uuid::Uuid;

use super::table_context_menu;
use super::view_detail_renderer;
use super::SidebarAction;

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
#[allow(clippy::too_many_arguments)]
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
    let folder_resp = egui::CollapsingHeader::new(label)
        .id_salt(format!("{conn_id}:{db_name}:{schema_name}:{kind}s"))
        .show(ui, |ui| {
            for view in views {
                let v_name = &view.name;
                let v_label = format!("{} {}", icon, v_name);

                let view_resp = egui::CollapsingHeader::new(&v_label)
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
                    });

                table_context_menu::render_view_context_menu(
                    &view_resp.header_response,
                    conn_id,
                    db_name,
                    schema_name,
                    v_name,
                    open_label,
                    action,
                );
                view_resp
                    .header_response
                    .on_hover_cursor(CursorIcon::PointingHand);
            }
        });
    folder_resp
        .header_response
        .on_hover_cursor(CursorIcon::PointingHand);
}
