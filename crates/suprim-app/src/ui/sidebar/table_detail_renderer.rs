use eframe::egui::{self, CursorIcon};
use suprim_core::db::types::{ColumnNode, ForeignKeyNode, IndexNode, TableNode};
use uuid::Uuid;

/// Render the detail tree under a single table node: Columns, Indexes, Foreign Keys.
pub(super) fn render_table_detail(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    table: &TableNode,
) {
    render_columns_folder(
        ui,
        conn_id,
        db_name,
        schema_name,
        &table.name,
        &table.columns,
    );
    render_indexes_folder(
        ui,
        conn_id,
        db_name,
        schema_name,
        &table.name,
        &table.indexes,
    );
    render_foreign_keys_folder(
        ui,
        conn_id,
        db_name,
        schema_name,
        &table.name,
        &table.foreign_keys,
    );
}

// ─── Columns ────────────────────────────────────────────────────────────────

fn render_columns_folder(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    table_name: &str,
    columns: &[ColumnNode],
) {
    if columns.is_empty() {
        return;
    }

    let label = format!(
        "{} Columns ({})",
        egui_phosphor::regular::COLUMNS,
        columns.len()
    );
    egui::CollapsingHeader::new(label)
        .id_salt(format!(
            "{conn_id}:{db_name}:{schema_name}:{table_name}:cols"
        ))
        .show(ui, |ui| {
            for col in columns {
                render_column_row(ui, col);
            }
        })
        .header_response
        .on_hover_cursor(CursorIcon::PointingHand);
}

fn render_column_row(ui: &mut egui::Ui, col: &ColumnNode) {
    let pk_marker = if col.is_primary_key { " PK" } else { "" };
    let null_marker = if col.nullable { "" } else { ", NOT NULL" };

    let display = format!("{} ({}){}{}", col.name, col.db_type, pk_marker, null_marker,);

    let color = if col.is_primary_key {
        ui.visuals().warn_fg_color
    } else {
        ui.visuals().text_color()
    };

    let icon = if col.is_primary_key {
        egui_phosphor::regular::KEY
    } else {
        egui_phosphor::regular::COLUMNS
    };

    ui.horizontal(|ui| {
        ui.colored_label(color, icon.to_string());
        ui.colored_label(color, display);
    });
}

// ─── Indexes ────────────────────────────────────────────────────────────────

fn render_indexes_folder(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    table_name: &str,
    indexes: &[IndexNode],
) {
    if indexes.is_empty() {
        return;
    }

    let label = format!(
        "{} Indexes ({})",
        egui_phosphor::regular::MAGNIFYING_GLASS,
        indexes.len()
    );
    egui::CollapsingHeader::new(label)
        .id_salt(format!(
            "{conn_id}:{db_name}:{schema_name}:{table_name}:idx"
        ))
        .show(ui, |ui| {
            for idx in indexes {
                let unique_tag = if idx.is_unique { " UNIQUE" } else { "" };
                let cols = idx.columns.join(", ");
                let display = format!("{}{} ({})", idx.name, unique_tag, cols);
                ui.label(display);
            }
        })
        .header_response
        .on_hover_cursor(CursorIcon::PointingHand);
}

// ─── Foreign Keys ───────────────────────────────────────────────────────────

fn render_foreign_keys_folder(
    ui: &mut egui::Ui,
    conn_id: Uuid,
    db_name: &str,
    schema_name: &str,
    table_name: &str,
    foreign_keys: &[ForeignKeyNode],
) {
    if foreign_keys.is_empty() {
        return;
    }

    let label = format!(
        "{} Foreign Keys ({})",
        egui_phosphor::regular::LINK,
        foreign_keys.len()
    );
    egui::CollapsingHeader::new(label)
        .id_salt(format!("{conn_id}:{db_name}:{schema_name}:{table_name}:fk"))
        .show(ui, |ui| {
            for fk in foreign_keys {
                let src_cols = fk.columns.join(", ");
                let ref_cols = fk.ref_columns.join(", ");
                let display = format!(
                    "{}: ({}) -> {}.{}",
                    fk.name, src_cols, fk.ref_table, ref_cols
                );
                ui.label(display);
            }
        })
        .header_response
        .on_hover_cursor(CursorIcon::PointingHand);
}
