use eframe::egui::{self, CursorIcon};
use suprim_core::db::types::{ColumnNode, ForeignKeyNode, IndexNode, TableNode};
use uuid::Uuid;

use crate::ui::icons;

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

    let state_id = ui.make_persistent_id(format!(
        "{conn_id}:{db_name}:{schema_name}:{table_name}:cols"
    ));
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), state_id, false);

    let (toggle_resp, header_resp, _body_resp) = state
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icons::db::column(icons::SIDEBAR_ICON, icons::db::COLOR_COLUMN));
                ui.label(format!("Columns ({})", columns.len()))
            })
        })
        .body(|ui| {
            for col in columns {
                render_column_row(ui, col);
            }
        });
    toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
    header_resp
        .inner
        .response
        .on_hover_cursor(CursorIcon::PointingHand);
}

fn render_column_row(ui: &mut egui::Ui, col: &ColumnNode) {
    let pk_marker = if col.is_primary_key { " PK" } else { "" };
    let null_marker = if col.nullable { "" } else { ", NOT NULL" };

    let display = format!("{} ({}){}{}", col.name, col.db_type, pk_marker, null_marker);

    let (icon, color) = if col.is_primary_key {
        (
            icons::db::key(icons::SIDEBAR_ICON, icons::db::COLOR_PK),
            icons::db::COLOR_PK,
        )
    } else {
        (
            icons::db::column(icons::SIDEBAR_ICON, icons::db::COLOR_COLUMN),
            icons::db::COLOR_COLUMN,
        )
    };

    ui.horizontal(|ui| {
        ui.label(icon);
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

    let state_id = ui.make_persistent_id(format!(
        "{conn_id}:{db_name}:{schema_name}:{table_name}:idx"
    ));
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), state_id, false);

    let (toggle_resp, header_resp, _body_resp) = state
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icons::db::index(icons::SIDEBAR_ICON, icons::db::COLOR_INDEX));
                ui.label(format!("Indexes ({})", indexes.len()))
            })
        })
        .body(|ui| {
            for idx in indexes {
                let unique_tag = if idx.is_unique { " UNIQUE" } else { "" };
                let cols = idx.columns.join(", ");
                let display = format!("{}{} ({})", idx.name, unique_tag, cols);
                ui.label(display);
            }
        });
    toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
    header_resp
        .inner
        .response
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

    let state_id =
        ui.make_persistent_id(format!("{conn_id}:{db_name}:{schema_name}:{table_name}:fk"));
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), state_id, false);

    let (toggle_resp, header_resp, _body_resp) = state
        .show_header(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icons::db::foreign_key(icons::SIDEBAR_ICON, icons::db::COLOR_FK));
                ui.label(format!("Foreign Keys ({})", foreign_keys.len()))
            })
        })
        .body(|ui| {
            for fk in foreign_keys {
                let src_cols = fk.columns.join(", ");
                let ref_cols = fk.ref_columns.join(", ");
                let display = format!(
                    "{}: ({}) -> {}.{}",
                    fk.name, src_cols, fk.ref_table, ref_cols
                );
                ui.label(display);
            }
        });
    toggle_resp.on_hover_cursor(CursorIcon::PointingHand);
    header_resp
        .inner
        .response
        .on_hover_cursor(CursorIcon::PointingHand);
}
