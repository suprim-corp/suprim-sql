//! Slow Queries table renderer for the Server Dashboard.
//!
//! Uses `egui_extras::TableBuilder` for proper column widths and left-aligned cells.

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use suprim_sql::db::schema::SlowQueryInfo;

const ROW_H: f32 = 22.0;

/// Render the Slow Queries table.
pub(super) fn render_slow_queries_table(ui: &mut egui::Ui, slow_queries: &[SlowQueryInfo]) {
    // Section header
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} Slow Queries  ({})",
                egui_phosphor::regular::HOURGLASS,
                slow_queries.len()
            ))
            .strong()
            .size(14.0),
        );
    });

    ui.add_space(4.0);

    if slow_queries.is_empty() {
        ui.label(egui::RichText::new("No slow queries").weak());
        return;
    }

    let num_rows = slow_queries.len();

    let builder = TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .max_scroll_height(200.0)
        .column(Column::remainder().clip(true)) // Query
        .column(Column::exact(60.0)) // Calls
        .column(Column::exact(80.0)) // Mean Time
        .column(Column::exact(80.0)) // Max Time
        .column(Column::exact(80.0)) // Total Time
        .column(Column::exact(70.0)); // Rows

    let table = builder.header(ROW_H, |mut header| {
        for hdr in [
            "Query",
            "Calls",
            "Mean Time",
            "Max Time",
            "Total Time",
            "Rows",
        ] {
            header.col(|ui| {
                ui.label(egui::RichText::new(hdr).weak().size(11.0));
            });
        }
    });

    table.body(|body| {
        body.rows(ROW_H, num_rows, |mut row| {
            let idx = row.index();
            let sq = &slow_queries[idx];

            // Query
            row.col(|ui| {
                let short = truncate_query(&sq.query, 60);
                let resp = ui.add(
                    egui::Label::new(egui::RichText::new(&short).monospace().size(10.0)).truncate(),
                );
                if sq.query.len() > 60 {
                    resp.on_hover_text(&sq.query);
                }
            });
            // Calls
            row.col(|ui| {
                ui.label(
                    egui::RichText::new(sq.calls.to_string())
                        .monospace()
                        .size(11.0),
                );
            });
            // Mean Time
            row.col(|ui| {
                ui.label(
                    egui::RichText::new(format_ms(sq.mean_time_ms))
                        .monospace()
                        .size(11.0),
                );
            });
            // Max Time
            row.col(|ui| {
                ui.label(
                    egui::RichText::new(format_ms(sq.max_time_ms))
                        .monospace()
                        .size(11.0),
                );
            });
            // Total Time
            row.col(|ui| {
                ui.label(
                    egui::RichText::new(format_ms(sq.total_time_ms))
                        .monospace()
                        .size(11.0),
                );
            });
            // Rows
            row.col(|ui| {
                ui.label(
                    egui::RichText::new(sq.rows.to_string())
                        .monospace()
                        .size(11.0),
                );
            });
        });
    });
}

/// Format milliseconds into human-readable (e.g. "1.23s", "456ms").
fn format_ms(ms: f64) -> String {
    if ms >= 1000.0 {
        format!("{:.2}s", ms / 1000.0)
    } else {
        format!("{:.0}ms", ms)
    }
}

fn truncate_query(q: &str, max: usize) -> String {
    let cleaned = q.replace('\n', " ").replace("  ", " ");
    if cleaned.len() <= max {
        cleaned
    } else {
        format!("{}...", &cleaned[..max])
    }
}
