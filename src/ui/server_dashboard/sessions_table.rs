//! Active Sessions table renderer for the Server Dashboard.
//!
//! Uses `egui_extras::TableBuilder` for proper column widths and left-aligned cells.

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use suprim_sql::db::commands::DbCommand;
use suprim_sql::db::schema::SessionInfo;
use tokio::sync::mpsc;
use uuid::Uuid;

const ROW_H: f32 = 22.0;

/// Render the Active Sessions table inside a scroll area.
pub(super) fn render_sessions_table(
    ui: &mut egui::Ui,
    sessions: &[SessionInfo],
    active_count: usize,
    conn_id: Uuid,
    cmd_tx: &mpsc::Sender<DbCommand>,
    max_height: f32,
) {
    // Section header
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{} Active Sessions ({})",
                egui_phosphor::regular::USERS,
                active_count
            ))
            .strong()
            .size(14.0),
        );
    });

    ui.add_space(4.0);

    if sessions.is_empty() {
        ui.label(egui::RichText::new("No sessions").weak());
        return;
    }

    let num_rows = sessions.len();

    let builder = TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .max_scroll_height(max_height - 40.0)
        .column(Column::exact(60.0)) // PID
        .column(Column::exact(90.0)) // User
        .column(Column::exact(120.0)) // Database
        .column(Column::exact(80.0)) // State
        .column(Column::exact(80.0)) // Duration
        .column(Column::remainder().clip(true)) // Query
        .column(Column::exact(28.0)); // Kill button

    let table = builder.header(ROW_H, |mut header| {
        for hdr in ["PID", "User", "Database", "State", "Duration", "Query", ""] {
            header.col(|ui| {
                ui.label(egui::RichText::new(hdr).weak().size(11.0));
            });
        }
    });

    table.body(|body| {
        body.rows(ROW_H, num_rows, |mut row| {
            let idx = row.index();
            let session = &sessions[idx];

            // PID
            row.col(|ui| {
                ui.label(
                    egui::RichText::new(session.pid.to_string())
                        .monospace()
                        .size(11.0),
                );
            });
            // User
            row.col(|ui| {
                ui.label(egui::RichText::new(&session.user).size(11.0));
            });
            // Database
            row.col(|ui| {
                ui.label(egui::RichText::new(&session.database).size(11.0));
            });
            // State
            row.col(|ui| {
                ui.label(state_label(&session.state));
            });
            // Duration
            row.col(|ui| {
                ui.label(
                    egui::RichText::new(&session.duration)
                        .monospace()
                        .size(11.0),
                );
            });
            // Query
            row.col(|ui| {
                let query_short = truncate_query(&session.query, 60);
                let resp = ui.add(
                    egui::Label::new(egui::RichText::new(&query_short).monospace().size(10.0))
                        .truncate(),
                );
                if session.query.len() > 60 {
                    resp.on_hover_text(&session.query);
                }
            });
            // Kill button
            row.col(|ui| {
                if session.state == "active"
                    && ui
                        .button(
                            egui::RichText::new(egui_phosphor::regular::X_CIRCLE)
                                .size(13.0)
                                .color(egui::Color32::from_rgb(200, 80, 80)),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(format!("Kill PID {}", session.pid))
                        .clicked()
                {
                    let _ = cmd_tx.try_send(DbCommand::KillSession {
                        conn_id,
                        pid: session.pid,
                    });
                }
            });
        });
    });
}

fn state_label(state: &str) -> egui::RichText {
    let (color, text) = match state {
        "active" => (egui::Color32::from_rgb(80, 180, 80), "active"),
        "idle" => (egui::Color32::from_rgb(150, 150, 150), "idle"),
        "idle in transaction" => (egui::Color32::from_rgb(200, 180, 60), "idle in tx"),
        "idle in transaction (aborted)" => {
            (egui::Color32::from_rgb(200, 80, 80), "idle in tx (err)")
        }
        _ => (egui::Color32::from_rgb(150, 150, 150), state),
    };
    egui::RichText::new(text).color(color).size(11.0)
}

fn truncate_query(q: &str, max: usize) -> String {
    let cleaned = q.replace('\n', " ").replace("  ", " ");
    if cleaned.len() <= max {
        cleaned
    } else {
        format!("{}...", &cleaned[..max])
    }
}
