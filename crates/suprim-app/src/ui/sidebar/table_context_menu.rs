use eframe::egui;
use suprim_core::db::types::TableNode;
use uuid::Uuid;

use super::SidebarAction;

/// Shorthand: button with pointer cursor on hover.
fn btn(ui: &mut egui::Ui, label: impl Into<egui::WidgetText>) -> egui::Response {
    ui.button(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Render the right-click context menu for a table header.
///
/// Menu layout (matches DBeaver-style):
///   View Data
///   Edit Table
///   Refresh
///   Export  ▸
///   Import  ▸
///   ─────────
///   Rename
///   Truncate
///   Delete
pub(super) fn render_table_context_menu(
    response: &egui::Response,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    table: &TableNode,
    schema_functions: &[String],
    action: &mut Option<SidebarAction>,
) {
    let table_name = &table.name;
    response.context_menu(|ui| {
        // ── Data Operations ─────────────────────────────────────────
        if btn(ui, "View Data").clicked() {
            *action = Some(SidebarAction::OpenTableViewer {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
                table_name: table_name.to_owned(),
            });
            ui.close();
        }

        if btn(ui, "Edit Table").clicked() {
            *action = Some(SidebarAction::EditTable {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
                table: table.clone(),
                schema_functions: schema_functions.to_vec(),
            });
            ui.close();
        }

        if btn(ui, "Refresh").clicked() {
            *action = Some(SidebarAction::RefreshSchema {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
            });
            ui.close();
        }

        // ── Export/Import submenus ──────────────────────────────────
        ui.menu_button("Export", |ui| {
            if btn(ui, "CSV").clicked() {
                // TODO: implement export CSV
                ui.close();
            }
            if btn(ui, "JSON").clicked() {
                // TODO: implement export JSON
                ui.close();
            }
            if btn(ui, "SQL INSERT").clicked() {
                // TODO: implement export SQL
                ui.close();
            }
        });

        ui.menu_button("Import", |ui| {
            if btn(ui, "CSV").clicked() {
                // TODO: implement import CSV
                ui.close();
            }
            if btn(ui, "JSON").clicked() {
                // TODO: implement import JSON
                ui.close();
            }
        });

        // ── Destructive Operations ──────────────────────────────────
        ui.separator();

        if btn(ui, "Rename").clicked() {
            // TODO: open rename dialog
            ui.close();
        }

        let truncate_label = egui::RichText::new("Truncate").color(ui.visuals().warn_fg_color);
        if btn(ui, truncate_label).clicked() {
            *action = Some(SidebarAction::TruncateTable {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
                table_name: table_name.to_owned(),
            });
            ui.close();
        }

        let delete_label =
            egui::RichText::new("Delete").color(egui::Color32::from_rgb(220, 60, 60));
        if btn(ui, delete_label).clicked() {
            *action = Some(SidebarAction::DropTable {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
                table_name: table_name.to_owned(),
            });
            ui.close();
        }
    });
}

/// Render the right-click context menu for a view/materialized view header.
///
/// Menu layout:
///   View Data
///   Refresh
///   Export  ▸
///   ─────────
///   Delete
pub(super) fn render_view_context_menu(
    response: &egui::Response,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    view_name: &str,
    open_label: &str,
    action: &mut Option<SidebarAction>,
) {
    response.context_menu(|ui| {
        if btn(ui, open_label).clicked() {
            *action = Some(SidebarAction::OpenTableViewer {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
                table_name: view_name.to_owned(),
            });
            ui.close();
        }

        if btn(ui, "Refresh").clicked() {
            *action = Some(SidebarAction::RefreshSchema {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
            });
            ui.close();
        }

        ui.menu_button("Export", |ui| {
            if btn(ui, "CSV").clicked() {
                // TODO: implement export CSV
                ui.close();
            }
            if btn(ui, "JSON").clicked() {
                // TODO: implement export JSON
                ui.close();
            }
            if btn(ui, "SQL INSERT").clicked() {
                // TODO: implement export SQL
                ui.close();
            }
        });

        ui.separator();

        let delete_label =
            egui::RichText::new("Delete").color(egui::Color32::from_rgb(220, 60, 60));
        if btn(ui, delete_label).clicked() {
            *action = Some(SidebarAction::DropView {
                conn_id,
                database: db_name.to_owned(),
                schema_name: schema_name.to_owned(),
                view_name: view_name.to_owned(),
            });
            ui.close();
        }
    });
}
