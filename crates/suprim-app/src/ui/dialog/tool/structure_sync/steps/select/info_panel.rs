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

    // 3-column layout: source (45%) | arrow (10%) | target (45%)
    // Using painter-based absolute positioning to guarantee column alignment.
    let total_w = ui.available_width();
    let col_w = (total_w * 0.45).max(100.0);
    let mid_w = total_w * 0.10;
    let wrap_width = col_w - 8.0;
    let panel_left = ui.cursor().left();
    let mid_x = panel_left + col_w; // start of middle column
    let right_x = panel_left + col_w + mid_w; // start of right column
    let mid_center_x = mid_x + mid_w / 2.0; // center of arrow column

    let arrow_icon = egui_phosphor::regular::ARROW_RIGHT;
    let text_color = ui.visuals().text_color();
    let weak_color = ui.visuals().weak_text_color();
    let arrow_color = weak_color;
    let body_size = ui.style().text_styles[&egui::TextStyle::Body].size;
    let key_font = egui::FontId::new(body_size + 1.0, egui::FontFamily::Proportional);
    let val_font = egui::FontId::new(body_size, egui::FontFamily::Proportional);
    let arrow_font = egui::FontId::proportional(body_size);

    let separator_stroke = egui::Stroke::new(
        0.5,
        ui.visuals()
            .widgets
            .noninteractive
            .bg_stroke
            .color
            .linear_multiply(0.4),
    );

    let row_h = 20.0;

    // Heading row — painted at absolute positions
    {
        let y = ui.cursor().top();
        let painter = ui.painter();

        // Source "Information" label
        painter.text(
            egui::pos2(panel_left, y),
            egui::Align2::LEFT_TOP,
            "Information",
            egui::FontId::new(body_size + 1.0, egui::FontFamily::Proportional),
            src_color,
        );
        // Target "Information" label
        painter.text(
            egui::pos2(right_x, y),
            egui::Align2::LEFT_TOP,
            "Information",
            egui::FontId::new(body_size + 1.0, egui::FontFamily::Proportional),
            tgt_color,
        );
        // Reserve space for the heading
        ui.allocate_space(egui::vec2(total_w, row_h));
    }

    // Data rows
    for (key, src_val, tgt_val) in &rows {
        // Subtle separator — left column only
        let y = ui.cursor().top();
        ui.painter().line_segment(
            [egui::pos2(panel_left, y), egui::pos2(panel_left + col_w, y)],
            separator_stroke,
        );
        // Subtle separator — right column only
        ui.painter().line_segment(
            [egui::pos2(right_x, y), egui::pos2(right_x + col_w, y)],
            separator_stroke,
        );

        // Measure how tall each side will be via LayoutJob galley
        let src_job = info_layout_job(
            key, src_val, &key_font, &val_font, text_color, weak_color, wrap_width,
        );
        let tgt_job = info_layout_job(
            key, tgt_val, &key_font, &val_font, text_color, weak_color, wrap_width,
        );
        let src_galley = ui.painter().layout_job(src_job);
        let tgt_galley = ui.painter().layout_job(tgt_job);
        let max_h = src_galley.size().y.max(tgt_galley.size().y).max(row_h);

        let row_y = ui.cursor().top();
        let painter = ui.painter();

        // Source text
        painter.galley(egui::pos2(panel_left, row_y), src_galley, text_color);
        // Arrow (centered vertically)
        painter.text(
            egui::pos2(mid_center_x, row_y + max_h / 2.0),
            egui::Align2::CENTER_CENTER,
            arrow_icon,
            arrow_font.clone(),
            arrow_color,
        );
        // Target text
        painter.galley(egui::pos2(right_x, row_y), tgt_galley, text_color);

        // Reserve the row height
        ui.allocate_space(egui::vec2(total_w, max_h));
    }
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
    #[allow(clippy::type_complexity)]
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
