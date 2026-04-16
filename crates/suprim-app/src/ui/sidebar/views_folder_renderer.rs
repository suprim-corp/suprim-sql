use eframe::egui::{self, CursorIcon, RichText};
use suprim_core::db::types::ViewNode;
use uuid::Uuid;

use super::table_context_menu;
use super::view_detail_renderer;
use super::SidebarAction;
use crate::ui::icons;

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
        ViewIconKind::View,
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
        ViewIconKind::MaterializedView,
        "Open Materialized View",
    );
}

/// Which icon variant to use for views vs materialized views.
enum ViewIconKind {
    View,
    MaterializedView,
}

impl ViewIconKind {
    fn rich(&self, size: f32) -> RichText {
        match self {
            ViewIconKind::View => icons::db::view(size, icons::db::COLOR_VIEW),
            ViewIconKind::MaterializedView => {
                icons::ph::colored("squares-four", size, icons::db::COLOR_VIEW)
            }
        }
    }
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
    icon_kind: ViewIconKind,
    open_label: &str,
) {
    let folder_state_id =
        ui.make_persistent_id(format!("{conn_id}:{db_name}:{schema_name}:{kind}s"));
    let folder_state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        folder_state_id,
        false,
    );

    let (toggle_resp, header_resp, _body_resp) = folder_state
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icon_kind.rich(icons::SIDEBAR_ICON));
                ui.label(format!("{} ({})", folder_name, views.len()))
            })
        })
        .body(|ui| {
            for view in views {
                let v_name = &view.name;

                let view_state_id = ui.make_persistent_id(format!(
                    "{conn_id}:{db_name}:{schema_name}:{kind}:{v_name}"
                ));
                let view_state = egui::collapsing_header::CollapsingState::load_with_default_open(
                    ui.ctx(),
                    view_state_id,
                    false,
                );

                let (v_toggle, v_header, _v_body) = view_state
                    .show_header(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(icon_kind.rich(icons::SIDEBAR_ICON));
                            ui.label(v_name)
                        })
                    })
                    .body(|ui| {
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
                    &v_header.inner.response,
                    conn_id,
                    db_name,
                    schema_name,
                    v_name,
                    open_label,
                    action,
                );
                v_toggle.on_hover_cursor(CursorIcon::PointingHand);
                v_header
                    .inner
                    .response
                    .on_hover_cursor(CursorIcon::PointingHand);
            }
        });
    toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
    header_resp
        .inner
        .response
        .on_hover_cursor(CursorIcon::PointingHand);
}
