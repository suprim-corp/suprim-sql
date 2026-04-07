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

// ── Cell editor popup state ───────────────────────────────────────────────────

enum CellEditorAction {
    None,
    Save,
    Close,
}

struct CellEditor {
    row: usize,
    col: usize,
    column_name: String,
    original_value: String,
    edit_value: String,
    is_json: bool,
    json_error: Option<String>,
}

// ── SQL Editor tab ────────────────────────────────────────────────────────────

struct SqlEditorTab {
    conn_id: Option<Uuid>,
    sql_text: String,
    result: Option<QueryResult>,
    /// Pre-computed display strings for each cell — avoids per-frame allocations.
    display_cache: Vec<Vec<String>>,
    is_running: bool,
    /// Currently selected data cell (row_idx, col_idx) for highlight + copy.
    selected_cell: Option<(usize, usize)>,
}

impl SqlEditorTab {
    fn new(conn_id: Option<Uuid>) -> Self {
        Self {
            conn_id,
            sql_text: String::new(),
            result: None,
            display_cache: Vec::new(),
            is_running: false,
            selected_cell: None,
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
                let _ =
                    render_result_grid(ui, result, &self.display_cache, &mut self.selected_cell);
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
    /// Pre-computed display strings for each cell — avoids per-frame allocations.
    display_cache: Vec<Vec<String>>,
    page: usize,
    page_size: usize,
    is_loading: bool,
    /// True until the first load is dispatched (auto-load on open).
    needs_initial_load: bool,
    where_clause: String,
    order_clause: String,
    /// Currently selected data cell (row_idx, col_idx) for highlight + copy.
    selected_cell: Option<(usize, usize)>,
    /// Popup cell editor opened by double-click.
    cell_editor: Option<CellEditor>,
}
impl TableViewerTab {
    fn new(conn_id: Uuid, database: String, schema_name: String, table_name: String) -> Self {
        Self {
            conn_id,
            database,
            schema_name,
            table_name,
            result: None,
            display_cache: Vec::new(),
            page: 0,
            page_size: 100,
            is_loading: false,
            needs_initial_load: true,
            where_clause: String::new(),
            order_clause: String::new(),
            selected_cell: None,
            cell_editor: None,
        }
    }

    fn load(&mut self, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        let where_opt = {
            let w = self.where_clause.trim().to_string();
            if w.is_empty() {
                None
            } else {
                Some(w)
            }
        };
        let order_opt = {
            let o = self.order_clause.trim().to_string();
            if o.is_empty() {
                None
            } else {
                Some(o)
            }
        };
        let _ = cmd_tx.try_send(DbCommand::LoadTableData {
            conn_id: self.conn_id,
            tab_id,
            database: Some(self.database.clone()),
            schema: Some(self.schema_name.clone()),
            table: self.table_name.clone(),
            page: self.page as u32,
            page_size: self.page_size as u32,
            where_clause: where_opt,
            order_clause: order_opt,
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
                let dbl =
                    render_result_grid(ui, result, &self.display_cache, &mut self.selected_cell);
                // Double-click → open cell editor popup
                if let Some((row, col)) = dbl {
                    if let Some(col_meta) = result.columns.get(col) {
                        let db_val = result.rows.get(row).and_then(|r| r.get(col));
                        let (raw, is_json) = match db_val {
                            Some(suprim_sql::db::types::DbValue::Json(v)) => {
                                // Pretty-print JSON
                                let pretty = serde_json::to_string_pretty(v)
                                    .unwrap_or_else(|_| v.to_string());
                                (pretty, true)
                            }
                            Some(v) => {
                                let s = v.display();
                                // Also detect JSON-like text strings
                                let looks_json = s.starts_with('{') || s.starts_with('[');
                                if looks_json {
                                    if let Ok(parsed) =
                                        serde_json::from_str::<serde_json::Value>(&s)
                                    {
                                        let pretty = serde_json::to_string_pretty(&parsed)
                                            .unwrap_or(s.clone());
                                        (pretty, true)
                                    } else {
                                        (s, false)
                                    }
                                } else {
                                    (s, false)
                                }
                            }
                            None => (String::new(), false),
                        };
                        self.cell_editor = Some(CellEditor {
                            row,
                            col,
                            column_name: col_meta.name.clone(),
                            original_value: raw.clone(),
                            edit_value: raw,
                            is_json,
                            json_error: None,
                        });
                    }
                }
            } else if self.is_loading {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            }

            // ── Cell editor popup ──
            self.render_cell_editor_popup(ui, tab_id, cmd_tx);
        });
    }

    /// Render the cell-editor popup when active.
    fn render_cell_editor_popup(
        &mut self,
        ui: &mut egui::Ui,
        tab_id: Uuid,
        cmd_tx: &mpsc::Sender<DbCommand>,
    ) {
        let mut action = CellEditorAction::None;

        if let Some(editor) = &mut self.cell_editor {
            let mut open = true;
            let title = if editor.is_json {
                format!("Edit JSON: {}", &editor.column_name)
            } else {
                format!("Edit: {}", &editor.column_name)
            };
            let col_name = editor.column_name.clone();
            let is_json = editor.is_json;
            let default_w = if is_json { 520.0 } else { 420.0 };
            let default_h = if is_json { 380.0 } else { 260.0 };
            let min_h = 180.0;

            egui::Window::new(title)
                .open(&mut open)
                .resizable([true, true])
                .default_width(default_w)
                .default_height(default_h)
                .min_height(min_h)
                .pivot(egui::Align2::CENTER_CENTER)
                .default_pos(ui.ctx().screen_rect().center())
                .show(ui.ctx(), |ui| {
                    // Column name label
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Column: {col_name}"))
                                .small()
                                .color(ui.visuals().weak_text_color()),
                        );
                        if is_json {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new("JSON")
                                            .small()
                                            .color(egui::Color32::from_rgb(86, 156, 214)),
                                    );
                                },
                            );
                        }
                    });
                    ui.add_space(4.0);

                    // Editor area fills all available height minus buttons row (~38px)
                    let text_height = (ui.available_height() - 38.0).max(80.0);

                    if is_json {
                        // JSON editor with syntax highlighting via egui_code_editor
                        use egui_code_editor::{CodeEditor, ColorTheme, Syntax};

                        // Transparent bg ("none") so editor inherits window background
                        let theme = if ui.visuals().dark_mode {
                            ColorTheme {
                                name: "adaptive-dark",
                                dark: true,
                                bg: "none",
                                cursor: "#a89984",
                                selection: "#504945",
                                comments: "#928374",
                                functions: "#b8bb26",
                                keywords: "#fb4934",
                                literals: "#ebdbb2",
                                numerics: "#d3869b",
                                punctuation: "#fe8019",
                                strs: "#8ec07c",
                                types: "#fabd2f",
                                special: "#83a598",
                            }
                        } else {
                            ColorTheme {
                                name: "adaptive-light",
                                dark: false,
                                bg: "none",
                                cursor: "#7c6f64",
                                selection: "#d5c4a1",
                                comments: "#7c6f64",
                                functions: "#79740e",
                                keywords: "#9d0006",
                                literals: "#282828",
                                numerics: "#8f3f71",
                                punctuation: "#af3a03",
                                strs: "#427b58",
                                types: "#b57614",
                                special: "#af3a03",
                            }
                        };

                        let json_syntax = Syntax::new("json")
                            .with_case_sensitive(true)
                            .with_keywords(["true", "false", "null"])
                            .with_quotes(['"']);
                        egui::ScrollArea::vertical()
                            .max_height(text_height)
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                CodeEditor::default()
                                    .id_source("json_cell_editor")
                                    .with_rows(12)
                                    .with_fontsize(13.0)
                                    .with_theme(theme)
                                    .with_syntax(json_syntax)
                                    .with_numlines(true)
                                    .vscroll(false)
                                    .show(ui, &mut editor.edit_value);
                            });
                    } else {
                        // Plain text editor
                        egui::ScrollArea::vertical()
                            .max_height(text_height)
                            .auto_shrink(false)
                            .show(ui, |ui| {
                                ui.add_sized(
                                    [ui.available_width(), text_height],
                                    egui::TextEdit::multiline(&mut editor.edit_value)
                                        .font(egui::TextStyle::Monospace),
                                );
                            });
                    }

                    // JSON validation error message
                    if let Some(err) = &editor.json_error {
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(err)
                                .small()
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                    }

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let changed = editor.edit_value != editor.original_value;
                        if is_json {
                            // Format button
                            if ui.button("Format").clicked() {
                                if let Ok(parsed) =
                                    serde_json::from_str::<serde_json::Value>(&editor.edit_value)
                                {
                                    editor.edit_value = serde_json::to_string_pretty(&parsed)
                                        .unwrap_or(editor.edit_value.clone());
                                    editor.json_error = None;
                                } else {
                                    editor.json_error = Some("Invalid JSON — cannot format".into());
                                }
                            }
                        }
                        if ui.add_enabled(changed, egui::Button::new("Save")).clicked() {
                            if is_json {
                                // Validate JSON before saving
                                match serde_json::from_str::<serde_json::Value>(&editor.edit_value)
                                {
                                    Ok(_) => {
                                        editor.json_error = None;
                                        action = CellEditorAction::Save;
                                    }
                                    Err(e) => {
                                        editor.json_error = Some(format!("Invalid JSON: {e}"));
                                    }
                                }
                            } else {
                                action = CellEditorAction::Save;
                            }
                        }
                        if ui.button("Cancel").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            action = CellEditorAction::Close;
                        }
                    });
                });
            if !open {
                action = CellEditorAction::Close;
            }
        }

        match action {
            CellEditorAction::Save => self.save_cell_edit(tab_id, cmd_tx),
            CellEditorAction::Close => self.cell_editor = None,
            CellEditorAction::None => {}
        }
    }

    /// Build and send an UpdateRow command from the current cell editor state.
    fn save_cell_edit(&mut self, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        let editor = match &self.cell_editor {
            Some(e) => e,
            None => return,
        };
        let result = match &self.result {
            Some(r) => r,
            None => return,
        };

        // Build primary key map from the first column (fallback: use all columns).
        // For now, use all column values of the row as the "where" key.
        let mut pk = std::collections::HashMap::new();
        if let Some(row_data) = result.rows.get(editor.row) {
            for (i, col) in result.columns.iter().enumerate() {
                if let Some(val) = row_data.get(i) {
                    pk.insert(col.name.clone(), val.clone());
                }
            }
        }

        let mut changes = std::collections::HashMap::new();
        changes.insert(
            editor.column_name.clone(),
            suprim_sql::db::types::DbValue::Text(editor.edit_value.clone()),
        );

        let schema_table = format!("\"{}\".\"{}\"", self.schema_name, self.table_name);

        let _ = cmd_tx.try_send(DbCommand::UpdateRow {
            conn_id: self.conn_id,
            tab_id,
            table: schema_table,
            pk,
            changes,
        });

        self.cell_editor = None;
        // Reload data to reflect update
        self.load(tab_id, cmd_tx);
    }
}

// ── Shared result grid renderer ───────────────────────────────────────────────

/// Pre-compute display strings for all cells once (avoids per-frame allocations).
fn build_display_cache(result: &QueryResult) -> Vec<Vec<String>> {
    result
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut cached = Vec::with_capacity(row.len() + 1);
            // First entry is the row number (1-indexed).
            cached.push(format!("{}", i + 1));
            for v in row {
                cached.push(v.display());
            }
            cached
        })
        .collect()
}

/// Render a fixed-width, left-aligned, clipped cell inside a `horizontal` row.
/// Returns the allocated `Rect` so callers can detect clicks on it.
fn fixed_cell(
    ui: &mut egui::Ui,
    width: f32,
    height: f32,
    add_content: impl FnOnce(&mut egui::Ui),
) -> egui::Rect {
    let parent_clip = ui.clip_rect();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let clip = rect.intersect(parent_clip);
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    child.set_clip_rect(clip);
    child.with_layout(
        egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(false),
        |ui| {
            add_content(ui);
        },
    );
    rect
}

/// Returns `Some((row, col))` when a cell is double-clicked (for opening an editor).
fn render_result_grid(
    ui: &mut egui::Ui,
    result: &QueryResult,
    display_cache: &[Vec<String>],
    selected_cell: &mut Option<(usize, usize)>,
) -> Option<(usize, usize)> {
    let mut double_clicked_cell: Option<(usize, usize)> = None;
    let num_rows = result.rows.len();
    let num_cols = result.columns.len();
    let col_width: f32 = 160.0;
    let row_num_width: f32 = 40.0;
    let row_height: f32 = 22.0;

    let available = ui.available_height() - 28.0;
    let weak = ui.visuals().weak_text_color();
    let border = ui.visuals().widgets.noninteractive.bg_stroke.color;
    let selection_fill = ui.visuals().selection.bg_fill;

    let total_width = row_num_width + (num_cols as f32 * col_width);

    // Handle Cmd+C / Ctrl+C to copy selected cell raw value
    if let Some((row, col)) = *selected_cell {
        let copy_requested = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Copy)));
        if copy_requested {
            if let Some(db_val) = result.rows.get(row).and_then(|r| r.get(col)) {
                let raw = db_val.display();
                ui.ctx().copy_text(raw);
            }
        }
    }

    egui::ScrollArea::horizontal()
        .id_salt("result_hscroll")
        .show(ui, |ui| {
            ui.set_min_width(total_width);

            // ── Header row ──
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                fixed_cell(ui, row_num_width, row_height, |ui| {
                    ui.add_space(4.0);
                    ui.label(egui::RichText::new("#").strong().color(weak));
                });
                for col in &result.columns {
                    fixed_cell(ui, col_width, row_height, |ui| {
                        let rect = ui.max_rect();
                        ui.painter().vline(
                            rect.left(),
                            rect.y_range(),
                            egui::Stroke::new(1.0, border),
                        );
                        ui.add_space(6.0);
                        ui.add(
                            egui::Label::new(egui::RichText::new(&col.name).strong()).truncate(),
                        );
                    });
                }
            });

            // ── Separator ──
            let sep_rect = ui.available_rect_before_wrap();
            ui.painter().hline(
                sep_rect.left()..=sep_rect.left() + total_width,
                sep_rect.top(),
                egui::Stroke::new(1.0, border),
            );
            ui.add_space(1.0);

            // ── Data rows (virtual) ──
            egui::ScrollArea::vertical()
                .id_salt("result_vscroll")
                .max_height(available)
                .show_rows(ui, row_height, num_rows, |ui, row_range| {
                    for row_idx in row_range {
                        if let Some(cached_row) = display_cache.get(row_idx) {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                let row_num = cached_row.first().map(|s| s.as_str()).unwrap_or("");
                                fixed_cell(ui, row_num_width, row_height, |ui| {
                                    ui.add_space(4.0);
                                    ui.label(egui::RichText::new(row_num).color(weak));
                                });
                                for col_idx in 0..num_cols {
                                    let val = cached_row
                                        .get(col_idx + 1)
                                        .map(|s| s.as_str())
                                        .unwrap_or("");
                                    let is_selected = *selected_cell == Some((row_idx, col_idx));
                                    let cell_rect = fixed_cell(ui, col_width, row_height, |ui| {
                                        let rect = ui.max_rect();
                                        // Draw selection bg BEFORE text
                                        if is_selected {
                                            ui.painter().rect_filled(rect, 0.0, selection_fill);
                                        }
                                        ui.painter().vline(
                                            rect.left(),
                                            rect.y_range(),
                                            egui::Stroke::new(1.0, border),
                                        );
                                        ui.add_space(6.0);
                                        ui.add(egui::Label::new(val).selectable(false).truncate());
                                    });
                                    // Click detection + pointer cursor
                                    let pointer_pos = ui.input(|i| i.pointer.interact_pos());
                                    if let Some(pos) = pointer_pos {
                                        if cell_rect.contains(pos) {
                                            ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                                            if ui.input(|i| i.pointer.any_click()) {
                                                *selected_cell = Some((row_idx, col_idx));
                                            }
                                            if ui.input(|i| {
                                                i.pointer.button_double_clicked(
                                                    egui::PointerButton::Primary,
                                                )
                                            }) {
                                                double_clicked_cell = Some((row_idx, col_idx));
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                });
        });

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

    double_clicked_cell
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
                let cache = build_display_cache(&result);
                match &mut entry.kind {
                    TabKind::SqlEditor(t) => {
                        t.result = Some(result);
                        t.display_cache = cache;
                        t.is_running = false;
                    }
                    TabKind::TableViewer(t) => {
                        t.result = Some(result);
                        t.display_cache = cache;
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
