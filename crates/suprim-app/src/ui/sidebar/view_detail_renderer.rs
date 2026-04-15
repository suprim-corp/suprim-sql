use eframe::egui::{self, CursorIcon};
use suprim_sql::db::types::ViewNode;
use uuid::Uuid;

/// Render the column list under a single view or materialized view node.
pub(super) fn render_view_detail(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    kind: &str,
    view: &ViewNode,
) {
    if view.columns.is_empty() {
        return;
    }

    let label = format!(
        "{} Columns ({})",
        egui_phosphor::regular::COLUMNS,
        view.columns.len()
    );
    egui::CollapsingHeader::new(label)
        .id_salt(format!(
            "{conn_id}:{db_name}:{schema_name}:{kind}:{}:cols",
            view.name
        ))
        .show(ui, |ui| {
            for col in &view.columns {
                let null_marker = if col.nullable { "" } else { ", NOT NULL" };
                let display = format!("{} ({}){}", col.name, col.db_type, null_marker);
                ui.label(display);
            }
        })
        .header_response
        .on_hover_cursor(CursorIcon::PointingHand);
}
