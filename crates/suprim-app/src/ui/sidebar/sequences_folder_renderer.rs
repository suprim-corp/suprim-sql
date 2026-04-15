use eframe::egui::{self, CursorIcon};
use suprim_core::db::types::SchemaNode;
use uuid::Uuid;

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
    let seq_resp = egui::CollapsingHeader::new(label)
        .id_salt(format!("{conn_id}:{db_name}:{schema_name}:sequences"))
        .show(ui, |ui| {
            for seq in &schema_node.sequences {
                ui.label(&seq.name);
            }
        });
    seq_resp
        .header_response
        .on_hover_cursor(CursorIcon::PointingHand);
}
