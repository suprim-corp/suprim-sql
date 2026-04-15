//! Endpoint picker rendering for the Structure Synchronization dialog.
//!
//! Extracted from `structure_sync_renderer.rs` to keep each file focused.

use eframe::egui;

use crate::ui::dialog::tool::structure_sync::types::{ConnInfo, Endpoint};

/// Render read-only summary of selected endpoints (used in compare/results steps).
pub(crate) fn render_endpoint_summary(
    ui: &mut egui::Ui,
    connections: &[ConnInfo],
    source: &Endpoint,
    target: &Endpoint,
) {
    ui.columns(2, |cols| {
        render_summary_side(
            &mut cols[0],
            "Source",
            egui::Color32::from_rgb(66, 165, 245),
            connections,
            source,
        );
        render_summary_side(
            &mut cols[1],
            "Target",
            egui::Color32::from_rgb(76, 175, 80),
            connections,
            target,
        );
    });
}

fn render_summary_side(
    ui: &mut egui::Ui,
    label: &str,
    label_color: egui::Color32,
    connections: &[ConnInfo],
    endpoint: &Endpoint,
) {
    ui.label(
        egui::RichText::new(label)
            .color(label_color)
            .strong()
            .size(13.0),
    );

    let conn_label = connections
        .get(endpoint.conn_idx)
        .map(|c| c.label.as_str())
        .unwrap_or("—");
    let db_label = if endpoint.database.is_empty() {
        "—"
    } else {
        &endpoint.database
    };
    let schema_label = if endpoint.schema.is_empty() {
        "—"
    } else {
        &endpoint.schema
    };

    let weak = ui.visuals().weak_text_color();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(egui_phosphor::regular::DATABASE).color(weak));
        ui.label(conn_label);
        ui.label(egui::RichText::new("/").color(weak));
        ui.label(db_label);
        ui.label(egui::RichText::new("/").color(weak));
        ui.label(schema_label);
    });
}

/// Render source + target pickers side by side with a swap button in between.
pub(crate) fn render_endpoint_pickers(
    ui: &mut egui::Ui,
    connections: &[ConnInfo],
    source: &mut Endpoint,
    target: &mut Endpoint,
) {
    let total_w = ui.available_width();
    let col_w = (total_w * 0.45).max(100.0);
    let swap_w = total_w * 0.10;

    ui.horizontal(|ui| {
        // Source column
        ui.vertical(|ui| {
            ui.set_min_width(col_w);
            ui.set_max_width(col_w);
            render_single_picker(
                ui,
                "Source",
                egui::Color32::from_rgb(66, 165, 245),
                connections,
                source,
                "src",
            );
        });

        // Swap button (centered in middle column)
        ui.vertical(|ui| {
            ui.set_min_width(swap_w);
            ui.set_max_width(swap_w);
            // Push down to roughly center the button within the picker height
            ui.add_space(40.0);
            ui.centered_and_justified(|ui| {
                let btn = ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(egui_phosphor::regular::ARROWS_LEFT_RIGHT)
                                .size(16.0),
                        )
                        .frame(false),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .on_hover_text("Swap source ↔ target");
                if btn.clicked() {
                    std::mem::swap(source, target);
                }
            });
        });

        // Target column
        ui.vertical(|ui| {
            ui.set_min_width(col_w);
            ui.set_max_width(col_w);
            render_single_picker(
                ui,
                "Target",
                egui::Color32::from_rgb(76, 175, 80),
                connections,
                target,
                "tgt",
            );
        });
    });
}

fn render_single_picker(
    ui: &mut egui::Ui,
    label: &str,
    label_color: egui::Color32,
    connections: &[ConnInfo],
    endpoint: &mut Endpoint,
    id_salt: &str,
) {
    ui.label(
        egui::RichText::new(label)
            .color(label_color)
            .strong()
            .size(14.0),
    );
    ui.add_space(4.0);

    let combo_width = (ui.available_width() - 8.0).max(100.0);

    // ── Connection ──
    ui.label(egui::RichText::new("Connection").strong());
    let conn_text = connections
        .get(endpoint.conn_idx)
        .map(|c| format!("{} {}", egui_phosphor::regular::DATABASE, c.label))
        .unwrap_or_else(|| "(none)".into());
    let conn_combo = egui::ComboBox::from_id_salt(format!("{id_salt}_conn"))
        .selected_text(&conn_text)
        .width(combo_width)
        .show_ui(ui, |ui| {
            for (i, conn) in connections.iter().enumerate() {
                if ui
                    .selectable_label(
                        endpoint.conn_idx == i,
                        format!("{} {}", egui_phosphor::regular::DATABASE, conn.label),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    endpoint.conn_idx = i;
                    endpoint.database = conn
                        .databases
                        .first()
                        .map(|d| d.name.clone())
                        .unwrap_or_default();
                    endpoint.schema = conn
                        .databases
                        .first()
                        .and_then(|d| d.schemas.first().cloned())
                        .unwrap_or_default();
                }
            }
        });
    conn_combo
        .response
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    ui.add_space(2.0);

    // ── Database ──
    ui.label(egui::RichText::new("Database").strong());
    let databases = connections
        .get(endpoint.conn_idx)
        .map(|c| c.databases.as_slice())
        .unwrap_or(&[]);
    let db_text = if endpoint.database.is_empty() {
        "(select database)".into()
    } else {
        format!(
            "{} {}",
            egui_phosphor::regular::HARD_DRIVES,
            endpoint.database
        )
    };
    let db_combo = egui::ComboBox::from_id_salt(format!("{id_salt}_db"))
        .selected_text(&db_text)
        .width(combo_width)
        .show_ui(ui, |ui| {
            for db in databases {
                if ui
                    .selectable_label(
                        endpoint.database == db.name,
                        format!("{} {}", egui_phosphor::regular::HARD_DRIVES, db.name),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    endpoint.database = db.name.clone();
                    endpoint.schema = db.schemas.first().cloned().unwrap_or_default();
                }
            }
        });
    db_combo
        .response
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    ui.add_space(2.0);

    // ── Schema ──
    ui.label(egui::RichText::new("Schema").strong());
    let schemas: &[String] = databases
        .iter()
        .find(|d| d.name == endpoint.database)
        .map(|d| d.schemas.as_slice())
        .unwrap_or(&[]);
    let schema_text = if endpoint.schema.is_empty() {
        "(select schema)".into()
    } else {
        format!(
            "{} {}",
            egui_phosphor::regular::TREE_STRUCTURE,
            endpoint.schema
        )
    };
    let schema_combo = egui::ComboBox::from_id_salt(format!("{id_salt}_schema"))
        .selected_text(&schema_text)
        .width(combo_width)
        .show_ui(ui, |ui| {
            for s in schemas {
                if ui
                    .selectable_label(
                        endpoint.schema == *s,
                        format!("{} {s}", egui_phosphor::regular::TREE_STRUCTURE),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    endpoint.schema = s.clone();
                }
            }
        });
    schema_combo
        .response
        .on_hover_cursor(egui::CursorIcon::PointingHand);
}
