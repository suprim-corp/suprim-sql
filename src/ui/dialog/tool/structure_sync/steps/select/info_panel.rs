//! Information panels for the Structure Synchronization dialog.
//!
//! Renders a two-column grid showing connection metadata (type, version, host,
//! port, database, schema) for source and target endpoints side-by-side.

use eframe::egui;

use crate::ui::dialog::tool::structure_sync::types::{ConnInfo, Endpoint};

// ── Information panels ──────────────────────────────────────────────────

pub(crate) fn render_information_panels(
    ui: &mut egui::Ui,
    connections: &[ConnInfo],
    source: &Endpoint,
    target: &Endpoint,
    status: &Option<String>,
) {
    if let Some(msg) = status {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(msg).color(ui.visuals().warn_fg_color));
        ui.add_space(4.0);
    }

    let src_conn = connections.get(source.conn_idx);
    let tgt_conn = connections.get(target.conn_idx);
    let rows = build_info_rows(src_conn, source, tgt_conn, target);

    let src_color = egui::Color32::from_rgb(66, 165, 245);
    let tgt_color = egui::Color32::from_rgb(76, 175, 80);

    // Use egui::Grid — each row automatically gets the max height of its cells.
    let col_width = (ui.available_width() - ui.spacing().item_spacing.x) / 2.0;

    egui::Grid::new("info_grid")
        .num_columns(2)
        .min_col_width(col_width)
        .max_col_width(col_width)
        .spacing([ui.spacing().item_spacing.x, 2.0])
        .show(ui, |ui| {
            // Heading row
            ui.label(egui::RichText::new("Information").color(src_color).strong());
            ui.label(egui::RichText::new("Information").color(tgt_color).strong());
            ui.end_row();

            let text_color = ui.visuals().text_color();
            let weak_color = ui.visuals().weak_text_color();
            let body_size = ui.style().text_styles[&egui::TextStyle::Body].size;
            let key_font = egui::FontId::new(body_size + 1.0, egui::FontFamily::Proportional);
            let val_font = egui::FontId::new(body_size, egui::FontFamily::Proportional);
            let wrap_width = col_width - 8.0;

            // Data rows — single Label with LayoutJob per cell
            let separator_stroke = egui::Stroke::new(
                0.5,
                ui.visuals()
                    .widgets
                    .noninteractive
                    .bg_stroke
                    .color
                    .linear_multiply(0.4),
            );

            for (key, src_val, tgt_val) in &rows {
                // Subtle separator between rows
                let rect = ui.available_rect_before_wrap();
                ui.painter().line_segment(
                    [
                        egui::pos2(rect.left(), rect.top()),
                        egui::pos2(
                            rect.left() + col_width * 2.0 + ui.spacing().item_spacing.x,
                            rect.top(),
                        ),
                    ],
                    separator_stroke,
                );

                // Align to top within the grid cell
                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    ui.add(
                        egui::Label::new(info_layout_job(
                            key, src_val, &key_font, &val_font, text_color, weak_color, wrap_width,
                        ))
                        .wrap(),
                    );
                });
                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    ui.add(
                        egui::Label::new(info_layout_job(
                            key, tgt_val, &key_font, &val_font, text_color, weak_color, wrap_width,
                        ))
                        .wrap(),
                    );
                });
                ui.end_row();
            }
        });
}

/// Build a LayoutJob: emphasized key + normal value in one text block.
fn info_layout_job(
    key: &str,
    value: &str,
    key_font: &egui::FontId,
    val_font: &egui::FontId,
    key_color: egui::Color32,
    val_color: egui::Color32,
    wrap_width: f32,
) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};

    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;

    // Emphasized key (larger font + strong color)
    job.append(
        &format!("{key}: "),
        0.0,
        TextFormat {
            font_id: key_font.clone(),
            color: key_color,
            ..Default::default()
        },
    );

    // Normal value
    job.append(
        value,
        0.0,
        TextFormat {
            font_id: val_font.clone(),
            color: val_color,
            ..Default::default()
        },
    );

    job
}

/// Build matched info rows for both sides.
fn build_info_rows(
    src_conn: Option<&ConnInfo>,
    source: &Endpoint,
    tgt_conn: Option<&ConnInfo>,
    target: &Endpoint,
) -> Vec<(&'static str, String, String)> {
    let keys: &[(&str, Box<dyn Fn(Option<&ConnInfo>, &Endpoint) -> String>)] = &[
        (
            "Database Type",
            Box::new(|c, _| c.map(|c| c.meta.driver_type.clone()).unwrap_or_default()),
        ),
        (
            "Version",
            Box::new(|c, _| {
                c.and_then(|c| c.meta.server_version.clone())
                    .unwrap_or_default()
            }),
        ),
        (
            "Name",
            Box::new(|c, _| c.map(|c| c.label.clone()).unwrap_or_default()),
        ),
        (
            "Host",
            Box::new(|c, _| c.map(|c| c.meta.host.clone()).unwrap_or_default()),
        ),
        (
            "Port",
            Box::new(|c, _| c.map(|c| c.meta.port.clone()).unwrap_or_default()),
        ),
        ("Database", Box::new(|_, ep| ep.database.clone())),
        ("Schema", Box::new(|_, ep| ep.schema.clone())),
    ];

    keys.iter()
        .filter_map(|(key, getter)| {
            let src = getter(src_conn, source);
            let tgt = getter(tgt_conn, target);
            if src.is_empty() && tgt.is_empty() {
                None
            } else {
                Some((*key, src, tgt))
            }
        })
        .collect()
}
