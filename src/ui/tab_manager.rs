use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::QueryResult;
use tokio::sync::mpsc;
use uuid::Uuid;

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

// ── SQL Editor tab ────────────────────────────────────────────────────────────

struct SqlEditorTab {
    conn_id: Option<Uuid>,
    sql_text: String,
    result: Option<QueryResult>,
    is_running: bool,
}

impl SqlEditorTab {
    fn new(conn_id: Option<Uuid>) -> Self {
        Self {
            conn_id,
            sql_text: String::new(),
            result: None,
            is_running: false,
        }
    }

    fn show(&mut self, ui: &mut egui::Ui, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        ui.vertical(|ui| {
            // Toolbar row
            ui.horizontal(|ui| {
                let run_btn = egui::Button::new(egui::RichText::new(format!(
                    "{} Run",
                    egui_phosphor::regular::PLAY
                )));
                let can_run = self.conn_id.is_some() && !self.is_running;
                if ui.add_enabled(can_run, run_btn).clicked() {
                    if let Some(conn_id) = self.conn_id {
                        let _ = cmd_tx.try_send(DbCommand::Execute {
                            conn_id,
                            tab_id,
                            sql: self.sql_text.clone(),
                        });
                        self.is_running = true;
                    }
                }

                if self.is_running {
                    ui.spinner();
                }

                if self.conn_id.is_none() {
                    ui.label(
                        egui::RichText::new("No connection selected").color(egui::Color32::YELLOW),
                    );
                }
            });

            ui.separator();

            // SQL text editor (top half)
            let available = ui.available_height();
            let editor_height = (available * 0.4).max(80.0);
            egui::ScrollArea::vertical()
                .id_salt("sql_editor_scroll")
                .max_height(editor_height)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.sql_text)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(10)
                            .desired_width(f32::INFINITY)
                            .hint_text("SELECT …"),
                    );
                });

            ui.separator();

            // Results grid (bottom half)
            if let Some(result) = &self.result {
                render_result_grid(ui, result);
            } else {
                let weak = ui.visuals().weak_text_color();
                ui.label(egui::RichText::new("Run a query to see results").color(weak));
            }
        });
    }
}

// ── Table Viewer tab ──────────────────────────────────────────────────────────

struct TableViewerTab {
    conn_id: Uuid,
    database: String,
    schema_name: String,
    table_name: String,
    result: Option<QueryResult>,
    page: usize,
    page_size: usize,
    is_loading: bool,
    /// True until the first load is dispatched (auto-load on open).
    needs_initial_load: bool,
    where_clause: String,
    order_clause: String,
}

impl TableViewerTab {
    fn new(conn_id: Uuid, database: String, schema_name: String, table_name: String) -> Self {
        Self {
            conn_id,
            database,
            schema_name,
            table_name,
            result: None,
            page: 0,
            page_size: 100,
            is_loading: false,
            needs_initial_load: true,
            where_clause: String::new(),
            order_clause: String::new(),
        }
    }

    fn load(&mut self, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        let _ = cmd_tx.try_send(DbCommand::LoadTableData {
            conn_id: self.conn_id,
            tab_id,
            database: Some(self.database.clone()),
            schema: Some(self.schema_name.clone()),
            table: self.table_name.clone(),
            page: self.page as u32,
            page_size: self.page_size as u32,
        });
        self.is_loading = true;
    }

    fn show(&mut self, ui: &mut egui::Ui, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        // Auto-load data on first render.
        if self.needs_initial_load {
            self.needs_initial_load = false;
            self.load(tab_id, cmd_tx);
        }

        // Derive colors from the current theme.
        let vis = ui.visuals().clone();
        let bar_bg = vis.faint_bg_color;
        let bar_stroke_color = vis.widgets.noninteractive.bg_stroke.color;
        let hint_color = vis.weak_text_color();

        ui.vertical(|ui| {
            // Filter bar — full width
            egui::Frame::NONE
                .fill(bar_bg)
                .stroke(egui::Stroke::new(1.0, bar_stroke_color))
                .inner_margin(egui::Margin::symmetric(4, 3))
                .show(ui, |ui| {
                    let _total_w = ui.available_width();
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;

                        // Reload button — left-most
                        if self.is_loading {
                            ui.spinner();
                        } else {
                            let resp = ui.add(
                                egui::Label::new(
                                    egui::RichText::new(egui_phosphor::regular::ARROW_CLOCKWISE)
                                        .color(hint_color)
                                        .size(16.0),
                                )
                                .selectable(false)
                                .sense(egui::Sense::click()),
                            );
                            if resp.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            if resp.clicked() {
                                self.page = 0;
                                self.load(tab_id, cmd_tx);
                            }
                        }

                        ui.separator();

                        // WHERE section — ~55% of remaining
                        let remaining = ui.available_width();
                        let where_w = (remaining * 0.55 - 50.0).max(80.0);
                        ui.label(egui::RichText::new("WHERE").color(hint_color).small());
                        let where_edit = egui::TextEdit::singleline(&mut self.where_clause)
                            .hint_text("e.g. id > 10")
                            .desired_width(where_w)
                            .frame(egui::Frame::NONE);
                        let where_resp = ui.add(where_edit);

                        ui.separator();

                        // ORDER BY section — rest
                        ui.label(egui::RichText::new("ORDER BY").color(hint_color).small());
                        let order_edit = egui::TextEdit::singleline(&mut self.order_clause)
                            .hint_text("e.g. id DESC")
                            .desired_width(ui.available_width())
                            .frame(egui::Frame::NONE);
                        let order_resp = ui.add(order_edit);

                        // Reload on Enter
                        let enter = where_resp.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        let enter2 = order_resp.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if enter || enter2 {
                            self.page = 0;
                            self.load(tab_id, cmd_tx);
                        }
                    });
                });

            // Pagination row (compact)
            if self.result.is_some() {
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button(egui_phosphor::regular::CARET_RIGHT)
                            .clicked()
                        {
                            self.page += 1;
                            self.load(tab_id, cmd_tx);
                        }
                        ui.label(
                            egui::RichText::new(format!("Page {}", self.page + 1))
                                .color(hint_color)
                                .small(),
                        );
                        if self.page > 0
                            && ui
                                .small_button(egui_phosphor::regular::CARET_LEFT)
                                .clicked()
                        {
                            self.page -= 1;
                            self.load(tab_id, cmd_tx);
                        }
                    });
                });
            }

            if let Some(result) = &self.result {
                render_result_grid(ui, result);
            } else if self.is_loading {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            }
        });
    }
}

// ── Shared result grid renderer ───────────────────────────────────────────────

fn render_result_grid(ui: &mut egui::Ui, result: &QueryResult) {
    let row_height = egui::TextStyle::Body.resolve(ui.style()).size + 6.0;
    let num_rows = result.rows.len();
    let num_cols = result.columns.len();
    let col_width: f32 = 150.0;

    // Reserve space for status bar at the bottom.
    let available = ui.available_height() - row_height - 8.0;

    // Sticky header — same column settings as data grid.
    egui::Grid::new("result_header")
        .min_col_width(col_width)
        .max_col_width(col_width)
        .show(ui, |ui| {
            for col in &result.columns {
                ui.add(egui::Label::new(egui::RichText::new(&col.name).strong()).truncate());
            }
            ui.end_row();
        });

    ui.separator();

    // Scrollable data rows.
    egui::ScrollArea::vertical()
        .id_salt("result_scroll")
        .max_height(available)
        .show(ui, |ui| {
            egui::Grid::new("result_grid")
                .striped(true)
                .min_col_width(col_width)
                .max_col_width(col_width)
                .show(ui, |ui| {
                    for row_idx in 0..num_rows {
                        let row = &result.rows[row_idx];
                        for col_idx in 0..num_cols {
                            let val = row.get(col_idx).map(|v| v.display()).unwrap_or_default();
                            ui.add(egui::Label::new(val).truncate());
                        }
                        ui.end_row();
                    }
                });
        });

    let weak = ui.visuals().weak_text_color();
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(format!(
            "{} rows  ·  {:.1} ms",
            num_rows,
            result.execution_time.as_secs_f64() * 1000.0
        ))
        .color(weak)
        .small(),
    );
}

// ── TabManager ────────────────────────────────────────────────────────────────

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
                match &mut entry.kind {
                    TabKind::SqlEditor(t) => {
                        t.result = Some(result);
                        t.is_running = false;
                    }
                    TabKind::TableViewer(t) => {
                        t.result = Some(result);
                        t.is_loading = false;
                    }
                }
                return;
            }
        }
    }

    pub fn on_row_mutated(&mut self, _tab_id: Uuid, _rows_affected: u64) {
        // Could refresh the table viewer here.
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
