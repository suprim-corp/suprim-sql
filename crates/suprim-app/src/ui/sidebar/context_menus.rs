//! Connection context menus and database picker — right-click actions on sidebar entries.

use eframe::egui::{self, CursorIcon};
use uuid::Uuid;

use super::connection_entry::ConnectionEntry;
use super::database_picker;
use super::sidebar_action::SidebarAction;

pub(super) fn render_disconnected_context_menu(
    header: &egui::Response,
    conn_id: Uuid,
    entry: &mut ConnectionEntry,
    action: &mut Option<SidebarAction>,
    _disconnect_id: &mut Option<Uuid>,
) {
    header.context_menu(|ui| {
        if ui
            .button("Connect")
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::Connect { conn_id });
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
        ui.separator();
        if ui
            .button("Delete Connection")
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::DeleteConnection {
                conn_id,
                conn_name: entry.label.clone(),
            });
            ui.close();
        }
    });
}

pub(super) fn render_context_menu(
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
        if ui
            .button("New Database...")
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::NewDatabase { conn_id });
            ui.close();
        }
        if ui
            .button("Server Dashboard")
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::OpenDashboard { conn_id });
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
        ui.separator();
        if ui
            .button("Delete Connection")
            .on_hover_cursor(CursorIcon::PointingHand)
            .clicked()
        {
            *action = Some(SidebarAction::DeleteConnection {
                conn_id,
                conn_name: entry.label.clone(),
            });
            ui.close();
        }
    });
}

pub(super) fn render_database_picker(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    entry: &mut ConnectionEntry,
    action: &mut Option<SidebarAction>,
) {
    if !entry.picker_open {
        return;
    }
    let label = super::sidebar_renderer::truncate_label(&entry.label, 24);
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
