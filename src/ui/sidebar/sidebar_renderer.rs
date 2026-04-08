use eframe::egui::{self, CursorIcon};
use uuid::Uuid;

use super::connection_entry::ConnectionEntry;
use super::sidebar_action::SidebarAction;
use super::{database_picker, schema_renderer};

/// Render all connections in the sidebar scroll area.
/// Returns an optional [`SidebarAction`] to be handled by the app.
pub(super) fn render_connections(
    ui: &mut egui::Ui,
    connections: &mut [ConnectionEntry],
) -> Option<SidebarAction> {
    let mut action: Option<SidebarAction> = None;
    let mut disconnect_id: Option<Uuid> = None;

    egui::ScrollArea::vertical()
        .auto_shrink(false)
        .show(ui, |ui| {
            for entry in connections.iter_mut() {
                render_single_connection(ui, entry, &mut action, &mut disconnect_id);
            }
        });

    if let Some(id) = disconnect_id {
        action = Some(SidebarAction::Disconnect { conn_id: id });
    }
    action
}

fn render_single_connection(
    ui: &mut egui::Ui,
    entry: &mut ConnectionEntry,
    action: &mut Option<SidebarAction>,
    disconnect_id: &mut Option<Uuid>,
) {
    let conn_id = entry.conn_id;
    let label = truncate_label(&entry.label, 24);
    let header = build_header_label(&label, entry.all_databases.len(), &entry.visible_databases);

    // Use CollapsingState directly so we can force-collapse
    // on the first render after a connection is created.
    let header_id = ui.make_persistent_id(format!("conn:{conn_id}"));
    let default_open = !entry.needs_collapse;
    if entry.needs_collapse {
        entry.needs_collapse = false;
    }
    let cs = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        header_id,
        default_open,
    );
    let resp = cs
        .show_header(ui, |ui| {
            ui.label(&header).on_hover_cursor(CursorIcon::PointingHand)
        })
        .body(|ui| {
            if let Some(schema) = &entry.schema {
                if let Some(a) = schema_renderer::render_schema_tree(
                    ui,
                    conn_id,
                    schema,
                    entry.visible_databases.as_ref(),
                    &mut entry.schema_detail_requested,
                    &mut entry.schemas_requested,
                ) {
                    *action = Some(a);
                }
            }
        });

    let (toggle_resp, header_resp, _body) = resp;

    // Click on label text also toggles expand/collapse.
    if header_resp.response.clicked() {
        if let Some(mut reloaded) =
            egui::collapsing_header::CollapsingState::load(ui.ctx(), header_id)
        {
            reloaded.toggle(ui);
            reloaded.store(ui.ctx());
        }
    }

    render_context_menu(&header_resp.inner, conn_id, entry, action, disconnect_id);
    render_context_menu(&toggle_resp, conn_id, entry, action, disconnect_id);
    toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
    header_resp
        .response
        .on_hover_cursor(CursorIcon::PointingHand);
    render_database_picker(ui, conn_id, entry, action);
}

fn render_context_menu(
    header: &egui::Response,
    conn_id: Uuid,
    entry: &mut ConnectionEntry,
    action: &mut Option<SidebarAction>,
    disconnect_id: &mut Option<Uuid>,
) {
    header.context_menu(|ui| {
        if ui
            .button("New SQL Tab")
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            let databases: Vec<String> =
                entry.all_databases.iter().map(|d| d.name.clone()).collect();
            *action = Some(SidebarAction::OpenSqlTab {
                conn_id,
                database: databases.first().cloned(),
                databases,
            });
            ui.close();
        }
        ui.separator();
        if ui
            .button("Filter Databases...")
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            entry.picker_open = !entry.picker_open;
            ui.close();
        }
        if ui
            .button("Edit Connection...")
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::EditConnection { conn_id });
            ui.close();
        }
        if ui
            .button("Disconnect")
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *disconnect_id = Some(conn_id);
            ui.close();
        }
    });
}

fn render_database_picker(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    entry: &mut ConnectionEntry,
    action: &mut Option<SidebarAction>,
) {
    if !entry.picker_open {
        return;
    }
    let label = truncate_label(&entry.label, 24);
    let picker_id = egui::Id::new(format!("db_picker_{conn_id}"));
    if let Some(new_visible) = database_picker::render_database_picker(
        ui.ctx(),
        &label,
        picker_id,
        &mut entry.picker_open,
        &entry.all_databases,
        &entry.visible_databases,
    ) {
        entry.visible_databases = new_visible.clone();
        if action.is_none() {
            *action = Some(SidebarAction::UpdateVisibleDatabases {
                conn_id,
                visible: new_visible,
            });
        }
    }
}

fn truncate_label(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        format!("{}...", chars[..max_chars - 1].iter().collect::<String>())
    }
}

fn build_header_label(label: &str, total: usize, visible: &Option<Vec<String>>) -> String {
    let badge = match visible {
        Some(v) => format!("{}/{}", v.len(), total),
        None => total.to_string(),
    };
    format!("{}  [{}]", label, badge)
}
