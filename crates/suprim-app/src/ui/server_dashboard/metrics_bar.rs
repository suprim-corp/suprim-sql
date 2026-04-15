//! Server Metrics card bar renderer for the Server Dashboard.

use eframe::egui;
use suprim_sql::db::schema::ServerMetrics;

/// Render the Server Metrics section as a row of equal-width cards.
pub(super) fn render_metrics_bar(ui: &mut egui::Ui, metrics: &ServerMetrics) {
    // Section header
    ui.label(
        egui::RichText::new(format!("{} Server Metrics", egui_phosphor::regular::GAUGE))
            .strong()
            .size(14.0),
    );

    ui.add_space(4.0);

    // 8 equal-width columns spanning full width
    let cards: [(&str, &str, String); 8] = [
        (
            egui_phosphor::regular::USERS,
            "Connected Threads",
            metrics.connected_sessions.to_string(),
        ),
        (
            egui_phosphor::regular::LIGHTNING,
            "Running Threads",
            metrics.active_queries.to_string(),
        ),
        (
            egui_phosphor::regular::CLOCK,
            "Uptime",
            metrics.uptime.clone(),
        ),
        (
            egui_phosphor::regular::ARROWS_CLOCKWISE,
            "Total Queries",
            format_large_number(metrics.total_transactions),
        ),
        (
            egui_phosphor::regular::HOURGLASS,
            "Slow Queries",
            metrics.slow_queries.to_string(),
        ),
        (
            egui_phosphor::regular::PLUGS_CONNECTED,
            "Max Connections",
            metrics.max_connections.to_string(),
        ),
        (
            egui_phosphor::regular::ARROW_FAT_DOWN,
            "Bytes Received",
            metrics.bytes_received.clone(),
        ),
        (
            egui_phosphor::regular::ARROW_FAT_UP,
            "Bytes Sent",
            metrics.bytes_sent.clone(),
        ),
    ];

    ui.columns(8, |cols| {
        for (i, (icon, label, value)) in cards.iter().enumerate() {
            metric_card(&mut cols[i], icon, label, value);
        }
    });
}

/// Render a single metric card (icon + label + value) that fills its column.
fn metric_card(ui: &mut egui::Ui, icon: &str, label: &str, value: &str) {
    let dark = ui.visuals().dark_mode;
    let bg = if dark {
        egui::Color32::from_rgb(40, 40, 45)
    } else {
        egui::Color32::from_rgb(245, 245, 247)
    };

    let avail_w = ui.available_width();

    egui::Frame::new()
        .fill(bg)
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(8, 8))
        .show(ui, |ui| {
            ui.set_min_width(avail_w - 16.0); // fill column minus frame margin
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).size(13.0).weak());
                    ui.label(egui::RichText::new(label).weak().size(11.0));
                });
                ui.label(egui::RichText::new(value).strong().size(14.0));
            });
        });
}

/// Format large numbers with separators (e.g. 161_103_246 → "161,103,246").
fn format_large_number(n: i64) -> String {
    if n < 1000 {
        return n.to_string();
    }
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}
