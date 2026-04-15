//! Renders the scrollable list of query history entries.

use eframe::egui;
use suprim_core::storage::QueryHistoryEntry;

use super::HistoryPanelOutput;

pub(super) fn render_history_list(
    ui: &mut egui::Ui,
    entries: &[&QueryHistoryEntry],
    output: &mut HistoryPanelOutput,
) {
    if entries.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new("No queries in history").weak());
        });
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("history_scroll")
        .show(ui, |ui| {
            for entry in entries {
                render_entry(ui, entry, output);
            }
        });
}

fn render_entry(ui: &mut egui::Ui, entry: &QueryHistoryEntry, output: &mut HistoryPanelOutput) {
    let frame = egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(8, 6))
        .corner_radius(4.0);

    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());

        // Row 1: status icon + SQL (truncated) + load button
        ui.horizontal(|ui| {
            // Success/failure icon
            if entry.success {
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::CHECK_CIRCLE)
                        .color(egui::Color32::from_rgb(80, 180, 80))
                        .size(14.0),
                );
            } else {
                ui.label(
                    egui::RichText::new(egui_phosphor::regular::X_CIRCLE)
                        .color(egui::Color32::from_rgb(220, 60, 60))
                        .size(14.0),
                );
            }

            // SQL text (truncated, monospace)
            let sql_display = truncate_sql(&entry.sql, 120);
            let sql_label = ui.add(
                egui::Label::new(egui::RichText::new(&sql_display).monospace().size(12.0))
                    .truncate()
                    .sense(egui::Sense::click()),
            );
            if sql_label.hovered() {
                sql_label.clone().on_hover_text(&entry.sql);
            }
            if sql_label.double_clicked() {
                output.load_sql = Some(entry.sql.clone());
            }
            if sql_label.double_clicked() {
                output.load_sql = Some(entry.sql.clone());
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(
                        egui::RichText::new(egui_phosphor::regular::ARROW_SQUARE_UP_RIGHT)
                            .size(14.0),
                    )
                    .on_hover_text("Load into editor")
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    output.load_sql = Some(entry.sql.clone());
                }
            });
        });

        // Row 2: metadata (connection, database, time, rows)
        ui.horizontal(|ui| {
            let weak = ui.visuals().weak_text_color();
            let meta_size = 10.5;

            // Connection name
            ui.label(
                egui::RichText::new(format!(
                    "{} {}",
                    egui_phosphor::regular::PLUGS_CONNECTED,
                    entry.conn_name
                ))
                .color(weak)
                .size(meta_size),
            );

            if let Some(db) = &entry.database {
                ui.label(
                    egui::RichText::new(format!("{} {}", egui_phosphor::regular::DATABASE, db))
                        .color(weak)
                        .size(meta_size),
                );
            }

            // Execution time
            if entry.success {
                ui.label(
                    egui::RichText::new(format!(
                        "{} {} ms",
                        egui_phosphor::regular::TIMER,
                        entry.execution_time_ms
                    ))
                    .color(weak)
                    .size(meta_size),
                );

                // Row count
                ui.label(
                    egui::RichText::new(format!(
                        "{} {} rows",
                        egui_phosphor::regular::ROWS,
                        entry.row_count
                    ))
                    .color(weak)
                    .size(meta_size),
                );
            } else if let Some(err) = &entry.error_message {
                let err_display = if err.len() > 60 {
                    format!("{}...", &err[..57])
                } else {
                    err.clone()
                };
                ui.label(
                    egui::RichText::new(err_display)
                        .color(egui::Color32::from_rgb(220, 80, 80))
                        .size(meta_size),
                );
            }

            // Timestamp (right-aligned)
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format_timestamp(&entry.timestamp))
                        .color(weak)
                        .size(meta_size),
                );
            });
        });
    });

    ui.separator();
}

/// Truncate SQL to a single line, replacing newlines with spaces.
fn truncate_sql(sql: &str, max_len: usize) -> String {
    let oneliner: String = sql
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let trimmed = oneliner.trim();
    if trimmed.len() > max_len {
        format!("{}...", &trimmed[..max_len])
    } else {
        trimmed.to_string()
    }
}

/// Format timestamp as relative time or absolute date.
fn format_timestamp(ts: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(*ts);

    if diff.num_seconds() < 60 {
        "just now".to_string()
    } else if diff.num_minutes() < 60 {
        format!("{}m ago", diff.num_minutes())
    } else if diff.num_hours() < 24 {
        format!("{}h ago", diff.num_hours())
    } else if diff.num_days() < 7 {
        format!("{}d ago", diff.num_days())
    } else {
        ts.format("%Y-%m-%d %H:%M").to_string()
    }
}
