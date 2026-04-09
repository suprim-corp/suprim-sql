//! Header banner, bottom bar, and re-exports for the Structure Sync dialog.
//!
//! Re-exports `render_endpoint_pickers` and `render_information_panels`
//! so `dialog.rs` can call everything through a single module.

use eframe::egui;

use super::types::{ConnInfo, Endpoint};

// Re-export so callers can still use `renderer::*`.
pub(crate) use super::endpoint_picker::render_endpoint_pickers;
pub(crate) use super::info_panel::render_information_panels;

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
