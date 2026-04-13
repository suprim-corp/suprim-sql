//! Server Dashboard tab — displays active sessions and server metrics.
//!
//! Submodules:
//!   - `sessions_table` — renders the Active Sessions table
//!   - `metrics_bar` — renders the Server Metrics cards

mod metrics_bar;
mod sessions_table;

use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::schema::{ServerMetrics, SessionInfo};
use tokio::sync::mpsc;
use uuid::Uuid;

/// Server Dashboard tab state.
pub struct ServerDashboardTab {
    pub conn_id: Uuid,
    sessions: Vec<SessionInfo>,
    metrics: ServerMetrics,
    is_loading: bool,
    /// Auto-refresh interval in seconds.
    refresh_interval: f32,
    /// Time of last data load (for auto-refresh).
    last_refresh: std::time::Instant,
    /// Whether auto-refresh is enabled.
    auto_refresh: bool,
}

impl ServerDashboardTab {
    pub fn new(conn_id: Uuid) -> Self {
        Self {
            conn_id,
            sessions: Vec::new(),
            metrics: ServerMetrics::default(),
            is_loading: true,
            refresh_interval: 5.0,
            last_refresh: std::time::Instant::now(),
            auto_refresh: true,
        }
    }

    /// Update dashboard data from DB event.
    pub fn on_data_loaded(&mut self, sessions: Vec<SessionInfo>, metrics: ServerMetrics) {
        self.sessions = sessions;
        self.metrics = metrics;
        self.is_loading = false;
        self.last_refresh = std::time::Instant::now();
    }

    /// Render the dashboard tab.
    pub fn show(&mut self, ui: &mut egui::Ui, cmd_tx: &mpsc::Sender<DbCommand>) {
        // Auto-refresh logic
        if self.auto_refresh {
            let elapsed = self.last_refresh.elapsed().as_secs_f32();
            if elapsed >= self.refresh_interval {
                self.request_refresh(cmd_tx);
            }
            // Schedule repaint for next refresh tick
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
        }

        // ── Toolbar ─────────────────────────────────────────────────────
        ui.horizontal(|ui| {
            // Refresh interval selector
            ui.label(egui::RichText::new(egui_phosphor::regular::CLOCK_CLOCKWISE).size(14.0));
            egui::ComboBox::from_id_salt("refresh_interval")
                .width(50.0)
                .selected_text(format!("{}s", self.refresh_interval as u32))
                .show_ui(ui, |ui| {
                    for secs in [1.0_f32, 2.0, 5.0, 10.0, 30.0, 60.0] {
                        ui.selectable_value(
                            &mut self.refresh_interval,
                            secs,
                            format!("{}s", secs as u32),
                        );
                    }
                });

            // Pause/Resume button
            let pause_icon = if self.auto_refresh {
                egui_phosphor::regular::PAUSE
            } else {
                egui_phosphor::regular::PLAY
            };
            if ui
                .button(egui::RichText::new(pause_icon).size(14.0))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(if self.auto_refresh {
                    "Pause auto-refresh"
                } else {
                    "Resume auto-refresh"
                })
                .clicked()
            {
                self.auto_refresh = !self.auto_refresh;
            }

            // Manual refresh button
            if ui
                .button(egui::RichText::new(egui_phosphor::regular::ARROW_CLOCKWISE).size(14.0))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Refresh now")
                .clicked()
            {
                self.request_refresh(cmd_tx);
            }

            if self.is_loading {
                ui.spinner();
            }
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // ── Layout: sessions scroll area + pinned metrics at bottom ─────
        // Reserve space for metrics bar at the bottom (~80px)
        const METRICS_H: f32 = 80.0;
        let sessions_h = (ui.available_height() - METRICS_H - 8.0).max(60.0);

        // Active Sessions section (scrollable)
        let active_count = self.sessions.iter().filter(|s| s.state == "active").count();
        sessions_table::render_sessions_table(
            ui,
            &self.sessions,
            active_count,
            self.conn_id,
            cmd_tx,
            sessions_h,
        );

        // Push metrics to the bottom
        let gap = (ui.available_height() - METRICS_H).max(0.0);
        if gap > 0.0 {
            ui.allocate_space(egui::vec2(0.0, gap));
        }

        // Server Metrics section (pinned at bottom)
        ui.separator();
        ui.add_space(4.0);
        metrics_bar::render_metrics_bar(ui, &self.metrics);
    }

    fn request_refresh(&mut self, cmd_tx: &mpsc::Sender<DbCommand>) {
        self.is_loading = true;
        self.last_refresh = std::time::Instant::now();
        let _ = cmd_tx.try_send(DbCommand::LoadDashboard {
            conn_id: self.conn_id,
        });
    }
}
