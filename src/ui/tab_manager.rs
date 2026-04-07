/// Tab manager — orchestrates tab bar rendering, tab lifecycle, and routing to
/// SqlEditorTab / TableViewerTab implementations.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::QueryResult;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::result_grid::build_display_cache;
use super::sql_editor_tab::SqlEditorTab;
use super::table_viewer_tab::TableViewerTab;

// ── Tab kinds ────────────────────────────────────────────────────────────────

enum TabKind {
    SqlEditor(SqlEditorTab),
    TableViewer(TableViewerTab),
}

// ── Tab entry ────────────────────────────────────────────────────────────────

struct TabEntry {
    tab_id: Uuid,
    kind: TabKind,
    conn_name: String,
}

impl TabEntry {
    fn tab_label(&self) -> String {
        let icon = match &self.kind {
            TabKind::SqlEditor(_) => egui_phosphor::regular::TERMINAL_WINDOW,
            TabKind::TableViewer(_) => egui_phosphor::regular::TABLE,
        };
        let name = match &self.kind {
            TabKind::SqlEditor(_) => "Query".to_string(),
            TabKind::TableViewer(t) => truncate_str(&t.table_name, 18),
        };
        let conn = truncate_str(&self.conn_name, 20);
        format!("{icon} {name} [{conn}]")
    }
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let half = max / 2;
        let start: String = s.chars().take(half).collect();
        let end: String = s
            .chars()
            .rev()
            .take(half)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("{start}...{end}")
    }
}

// ── TabManager ───────────────────────────────────────────────────────────────

pub struct TabManager {
    tabs: Vec<TabEntry>,
    active_tab: Option<Uuid>,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: None,
        }
    }

    pub fn open_sql_tab(&mut self, conn_id: Option<Uuid>, conn_name: String) {
        let tab_id = Uuid::new_v4();
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::SqlEditor(SqlEditorTab::new(conn_id)),
            conn_name,
        });
        self.active_tab = Some(tab_id);
    }

    pub fn open_table_viewer(
        &mut self,
        conn_id: Uuid,
        conn_name: String,
        database: String,
        schema_name: String,
        table_name: String,
    ) {
        let tab_id = Uuid::new_v4();
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::TableViewer(TableViewerTab::new(
                conn_id,
                database,
                schema_name,
                table_name,
            )),
            conn_name,
        });
        self.active_tab = Some(tab_id);
    }

    pub fn on_query_result(&mut self, tab_id: Uuid, result: QueryResult) {
        for entry in &mut self.tabs {
            if entry.tab_id == tab_id {
                let cache = build_display_cache(&result);
                match &mut entry.kind {
                    TabKind::SqlEditor(t) => t.set_result(result, cache),
                    TabKind::TableViewer(t) => t.set_result(result, cache),
                }
                return;
            }
        }
    }

    pub fn on_row_mutated(&mut self, _tab_id: Uuid, _rows_affected: u64) {
        // Could refresh the table viewer here.
    }

    /// Returns true if any tab is currently waiting for a DB response.
    pub fn any_tab_loading(&self) -> bool {
        self.tabs.iter().any(|entry| match &entry.kind {
            TabKind::SqlEditor(t) => t.is_running,
            TabKind::TableViewer(t) => t.is_loading,
        })
    }

    pub fn show(&mut self, ui: &mut egui::Ui, cmd_tx: &mpsc::Sender<DbCommand>) {
        if self.tabs.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    ui.heading("suprim-sql");
                    ui.add_space(12.0);
                    ui.label("Open a connection from the sidebar to get started.");
                });
            });
            return;
        }

        // Derive all tab-bar colors from the current theme.
        let vis = ui.visuals().clone();
        let bar_bg = vis.faint_bg_color;
        let active_bg = vis.widgets.active.bg_fill;
        let hover_bg = vis.widgets.hovered.bg_fill;
        let active_border = vis.widgets.active.bg_stroke.color;
        let hover_border = vis.widgets.hovered.bg_stroke.color;
        let inactive_border = vis.widgets.noninteractive.bg_stroke.color;
        let active_text = vis.strong_text_color();
        let inactive_text = vis.widgets.inactive.fg_stroke.color;
        let close_active_color = vis.widgets.inactive.fg_stroke.color;
        let close_inactive_color = vis.weak_text_color();

        // Tab bar
        let mut tab_to_close: Option<Uuid> = None;

        egui::Frame::NONE
            .fill(bar_bg)
            .inner_margin(egui::Margin::symmetric(2, 0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for entry in &self.tabs {
                        let is_active = Some(entry.tab_id) == self.active_tab;
                        let tab_id = entry.tab_id;
                        let label_text = entry.tab_label();

                        // Check hover from previous frame to adjust visuals
                        let tab_hover_id = egui::Id::new(("tab_hover", tab_id));
                        let was_hovered = ui
                            .ctx()
                            .data(|d| d.get_temp::<bool>(tab_hover_id).unwrap_or(false));

                        let actual_bg = if is_active {
                            active_bg
                        } else if was_hovered {
                            hover_bg
                        } else {
                            egui::Color32::TRANSPARENT
                        };
                        let actual_border = if is_active {
                            active_border
                        } else if was_hovered {
                            hover_border
                        } else {
                            inactive_border
                        };

                        let frame_response = egui::Frame::NONE
                            .fill(actual_bg)
                            .stroke(egui::Stroke::new(1.0, actual_border))
                            .inner_margin(egui::Margin::symmetric(6, 3))
                            .corner_radius(egui::CornerRadius::same(0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let text_color = if is_active {
                                        active_text
                                    } else {
                                        inactive_text
                                    };
                                    let response = ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&label_text).color(text_color),
                                        )
                                        .selectable(false)
                                        .sense(egui::Sense::click()),
                                    );
                                    if response.clicked() {
                                        self.active_tab = Some(tab_id);
                                    }

                                    ui.add_space(6.0);
                                    let close_color = if is_active {
                                        close_active_color
                                    } else {
                                        close_inactive_color
                                    };
                                    let close_response = ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(egui_phosphor::regular::X)
                                                .color(close_color),
                                        )
                                        .selectable(false)
                                        .sense(egui::Sense::click()),
                                    );
                                    if close_response.clicked() {
                                        tab_to_close = Some(tab_id);
                                    }
                                });
                            });

                        // Store hover state for next frame + set cursor
                        let tab_rect = frame_response.response.rect;
                        let is_hovered = ui.rect_contains_pointer(tab_rect);
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(tab_hover_id, is_hovered));
                        if is_hovered {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }
                });
            });

        ui.separator();

        // Close tab if requested
        if let Some(id) = tab_to_close {
            self.tabs.retain(|t| t.tab_id != id);
            if self.active_tab == Some(id) {
                self.active_tab = self.tabs.last().map(|t| t.tab_id);
            }
        }

        // Show active tab content
        if let Some(active_id) = self.active_tab {
            for entry in &mut self.tabs {
                if entry.tab_id == active_id {
                    let tab_id = entry.tab_id;
                    match &mut entry.kind {
                        TabKind::SqlEditor(t) => t.show(ui, tab_id, cmd_tx),
                        TabKind::TableViewer(t) => t.show(ui, tab_id, cmd_tx),
                    }
                    break;
                }
            }
        }
    }
}
