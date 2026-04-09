//! Rendering helpers for the Structure Synchronization dialog.
//!
//! All `render_*` methods live here to keep `structure_sync_dialog.rs` lean.

use eframe::egui;

use super::structure_sync_types::{ConnInfo, Endpoint};

pub(crate) fn endpoint_summary(connections: &[ConnInfo], ep: &Endpoint) -> (String, String) {
    let name = connections
        .get(ep.conn_idx)
        .map(|c| c.label.as_str())
        .unwrap_or("?");
    let db = if ep.database.is_empty() {
        "?"
    } else {
        &ep.database
    };
    let sch = if ep.schema.is_empty() {
        "?"
    } else {
        &ep.schema
    };
    (name.to_owned(), format!("{db}.{sch}"))
}

// ── Header banner ───────────────────────────────────────────────────────

pub(crate) fn render_header_banner(
    ui: &mut egui::Ui,
    connections: &[ConnInfo],
    source: &Endpoint,
    target: &Endpoint,
) {
    let (src_name, src_path) = endpoint_summary(connections, source);
    let (tgt_name, tgt_path) = endpoint_summary(connections, target);

    ui.add_space(4.0);

    let w = ui.available_width();
    let h = 40.0;

    let banner_resp = ui.allocate_ui(egui::vec2(w, h), |ui| {
        ui.columns(2, |cols| {
            // Source (right-aligned)
            cols[0].with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::DATABASE)
                        .size(22.0)
                        .color(egui::Color32::from_rgb(76, 175, 80)),
                );
                ui.vertical(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        ui.label(egui::RichText::new(&src_name).size(11.0))
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        ui.label(egui::RichText::new(&src_path).size(10.0).weak())
                    });
                });
            });

            // Target (left-aligned)
            cols[1].with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(14.0);
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::DATABASE)
                        .size(22.0)
                        .color(egui::Color32::from_rgb(66, 165, 245)),
                );
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&tgt_name).size(11.0));
                    ui.label(egui::RichText::new(&tgt_path).size(10.0).weak());
                });
            });
        });
    });

    // Paint arrow overlay at exact center of the banner rect
    let banner_rect = banner_resp.response.rect;
    let center = banner_rect.center();

    let galley = ui.painter().layout_no_wrap(
        egui_phosphor::regular::ARROW_RIGHT.to_owned(),
        egui::FontId::proportional(14.0),
        ui.visuals().weak_text_color(),
    );
    ui.painter().galley(
        egui::pos2(
            center.x - galley.size().x / 2.0,
            center.y - galley.size().y / 2.0,
        ),
        galley,
        ui.visuals().weak_text_color(),
    );

    ui.add_space(4.0);
}

// ── Endpoint pickers ────────────────────────────────────────────────────

pub(crate) fn render_endpoint_pickers(
    ui: &mut egui::Ui,
    connections: &[ConnInfo],
    source: &mut Endpoint,
    target: &mut Endpoint,
) {
    ui.columns(2, |cols| {
        render_single_picker(
            &mut cols[0],
            "Source",
            egui::Color32::from_rgb(66, 165, 245),
            connections,
            source,
            "src",
        );
        render_single_picker(
            &mut cols[1],
            "Target",
            egui::Color32::from_rgb(76, 175, 80),
            connections,
            target,
            "tgt",
        );
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

// ── Information panels ──────────────────────────────────────────────────

pub(crate) fn render_information_panels(
    ui: &mut egui::Ui,
    connections: &[ConnInfo],
    source: &Endpoint,
    target: &Endpoint,
    status: &Option<String>,
) {
    let info_height = 180.0;

    if let Some(msg) = status {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(msg).color(ui.visuals().warn_fg_color));
        ui.add_space(4.0);
    }

    ui.columns(2, |cols| {
        render_info_column(
            &mut cols[0],
            "Information",
            egui::Color32::from_rgb(66, 165, 245),
            connections,
            source,
            info_height,
            "src_info",
        );
        render_info_column(
            &mut cols[1],
            "Information",
            egui::Color32::from_rgb(76, 175, 80),
            connections,
            target,
            info_height,
            "tgt_info",
        );
    });
}

fn render_info_column(
    ui: &mut egui::Ui,
    title: &str,
    color: egui::Color32,
    connections: &[ConnInfo],
    endpoint: &Endpoint,
    max_height: f32,
    scroll_id: &str,
) {
    ui.label(egui::RichText::new(title).color(color).strong());
    ui.add_space(4.0);

    egui::ScrollArea::vertical()
        .id_salt(scroll_id)
        .max_height(max_height)
        .show(ui, |ui| {
            // Disable horizontal justify so wrapped text isn't stretched
            ui.with_layout(
                egui::Layout::top_down(egui::Align::LEFT).with_main_justify(false),
                |ui| {
                    if let Some(conn) = connections.get(endpoint.conn_idx) {
                        let m = &conn.meta;
                        info_row(ui, "Database Type", &m.driver_type);
                        if let Some(ver) = &m.server_version {
                            info_row(ui, "Version", ver);
                        }
                        info_row(ui, "Name", &conn.label);
                        if !m.host.is_empty() {
                            info_row(ui, "Host", &m.host);
                        }
                        if !m.port.is_empty() {
                            info_row(ui, "Port", &m.port);
                        }
                        if !endpoint.database.is_empty() {
                            info_row(ui, "Database", &endpoint.database);
                        }
                        if !endpoint.schema.is_empty() {
                            info_row(ui, "Schema", &endpoint.schema);
                        }
                    } else {
                        ui.label(egui::RichText::new("No connection selected.").weak());
                    }
                },
            );
        });
}

/// Single key-value row with the value wrapping (not justified).
fn info_row(ui: &mut egui::Ui, key: &str, value: &str) {
    let text = format!("{key}: {value}");
    ui.add(egui::Label::new(text).wrap());
}

// ── Bottom bar ──────────────────────────────────────────────────────────

pub(crate) fn render_bottom_bar(
    ui: &mut egui::Ui,
    compared: bool,
    ddl_script: &str,
    status: &mut Option<String>,
    open: &mut bool,
    run_compare: &mut bool,
) {
    ui.horizontal(|ui| {
        if ui
            .button("Options")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            // TODO: options dialog
        }

        if compared && !ddl_script.is_empty() {
            if ui
                .button(format!(
                    "{}  Copy Script",
                    egui_phosphor::regular::CLIPBOARD_TEXT
                ))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                ui.ctx().copy_text(ddl_script.to_owned());
                *status = Some("Script copied to clipboard".into());
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button("Compare")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *run_compare = true;
            }
            if ui
                .button("Close")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *open = false;
            }
        });
    });
}
