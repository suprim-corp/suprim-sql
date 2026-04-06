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

impl TabKind {
    fn title(&self) -> &str {
        match self {
            TabKind::SqlEditor(_) => "SQL Editor",
            TabKind::TableViewer(t) => &t.table_name,
        }
    }
}

// ── Tab entry ────────────────────────────────────────────────────────────────

struct TabEntry {
    tab_id: Uuid,
    kind: TabKind,
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
                let run_btn = egui::Button::new("▶ Run");
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
                ui.label(
                    egui::RichText::new("Run a query to see results").color(egui::Color32::GRAY),
                );
            }
        });
    }
}

// ── Table Viewer tab ──────────────────────────────────────────────────────────

struct TableViewerTab {
    conn_id: Uuid,
    table_name: String,
    result: Option<QueryResult>,
    page: usize,
    page_size: usize,
    is_loading: bool,
}

impl TableViewerTab {
    fn new(conn_id: Uuid, table_name: String) -> Self {
        Self {
            conn_id,
            table_name,
            result: None,
            page: 0,
            page_size: 100,
            is_loading: false,
        }
    }

    fn load(&mut self, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        let _ = cmd_tx.try_send(DbCommand::LoadTableData {
            conn_id: self.conn_id,
            tab_id,
            schema: None,
            table: self.table_name.clone(),
            page: self.page as u32,
            page_size: self.page_size as u32,
        });
        self.is_loading = true;
    }

    fn show(&mut self, ui: &mut egui::Ui, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        ui.vertical(|ui| {
            // Toolbar
            ui.horizontal(|ui| {
                ui.heading(&self.table_name);
                ui.add_space(8.0);
                if ui.button("🔄 Reload").clicked() {
                    self.load(tab_id, cmd_tx);
                }
                if self.is_loading {
                    ui.spinner();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("▶").clicked() {
                        self.page += 1;
                        self.load(tab_id, cmd_tx);
                    }
                    ui.label(format!("Page {}", self.page + 1));
                    if self.page > 0 && ui.button("◀").clicked() {
                        self.page -= 1;
                        self.load(tab_id, cmd_tx);
                    }
                });
            });

            ui.separator();

            if let Some(result) = &self.result {
                render_result_grid(ui, result);
            } else {
                ui.label(
                    egui::RichText::new("Click Reload to load table data")
                        .color(egui::Color32::GRAY),
                );
            }
        });
    }
}

// ── Shared result grid renderer ───────────────────────────────────────────────

fn render_result_grid(ui: &mut egui::Ui, result: &QueryResult) {
    let text_height = egui::TextStyle::Body.resolve(ui.style()).size + 4.0;

    egui::ScrollArea::both()
        .id_salt("result_scroll")
        .show(ui, |ui| {
            egui::Grid::new("result_grid")
                .striped(true)
                .min_col_width(60.0)
                .show(ui, |ui| {
                    // Header row
                    for col in &result.columns {
                        ui.strong(&col.name);
                    }
                    ui.end_row();

                    // Data rows
                    for row in &result.rows {
                        for val in row {
                            ui.label(val.display());
                        }
                        ui.end_row();
                    }
                });

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} rows  ·  {:.1} ms",
                    result.rows.len(),
                    result.execution_time.as_secs_f64() * 1000.0
                ))
                .color(egui::Color32::GRAY)
                .small(),
            );
        });
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

    pub fn open_sql_tab(&mut self, conn_id: Option<Uuid>) {
        let tab_id = Uuid::new_v4();
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::SqlEditor(SqlEditorTab::new(conn_id)),
        });
        self.active_tab = Some(tab_id);
    }

    pub fn open_table_viewer(&mut self, conn_id: Uuid, table_name: String) {
        let tab_id = Uuid::new_v4();
        self.tabs.push(TabEntry {
            tab_id,
            kind: TabKind::TableViewer(TableViewerTab::new(conn_id, table_name)),
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

        // Tab bar
        let mut tab_to_close: Option<Uuid> = None;

        ui.horizontal(|ui| {
            for entry in &self.tabs {
                let is_active = Some(entry.tab_id) == self.active_tab;
                let tab_label = entry.kind.title();

                let mut frame = egui::Frame::NONE;
                if is_active {
                    frame = frame.fill(ui.visuals().extreme_bg_color);
                }

                let (rect, response) = ui.allocate_at_least(
                    egui::vec2(
                        8.0 + tab_label.len() as f32 * 7.5 + 24.0,
                        ui.available_height(),
                    ),
                    egui::Sense::click(),
                );

                if response.clicked() {
                    self.active_tab = Some(entry.tab_id);
                }

                frame.show(ui, |ui| {
                    ui.set_min_width(rect.width());
                    ui.horizontal(|ui| {
                        ui.label(tab_label);
                        if ui.small_button("✕").clicked() {
                            tab_to_close = Some(entry.tab_id);
                        }
                    });
                });
            }
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
