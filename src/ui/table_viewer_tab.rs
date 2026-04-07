/// Table Viewer tab — browse table data with pagination, filtering, cell editing.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::QueryResult;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::result_grid::render_result_grid;

// ── Cell editor popup state ───────────────────────────────────────────────────

enum CellEditorAction {
    None,
    Save,
    Close,
}

struct CellEditor {
    row: usize,
    #[allow(dead_code)]
    col: usize,
    column_name: String,
    original_value: String,
    edit_value: String,
    is_json: bool,
    json_error: Option<String>,
}

// ── TableViewerTab ────────────────────────────────────────────────────────────

pub struct TableViewerTab {
    pub conn_id: Uuid,
    database: String,
    schema_name: String,
    pub table_name: String,
    result: Option<QueryResult>,
    /// Pre-computed display strings for each cell — avoids per-frame allocations.
    display_cache: Vec<Vec<String>>,
    page: usize,
    page_size: usize,
    /// Total row count from the DB (for pagination display).
    total_count: Option<u64>,
    pub is_loading: bool,
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
    pub fn new(conn_id: Uuid, database: String, schema_name: String, table_name: String) -> Self {
        Self {
            conn_id,
            database,
            schema_name,
            table_name,
            result: None,
            display_cache: Vec::new(),
            page: 0,
            page_size: 100,
            total_count: None,
            is_loading: false,
            needs_initial_load: true,
            where_clause: String::new(),
            order_clause: String::new(),
            selected_cell: None,
            cell_editor: None,
        }
    }

    pub fn set_result(&mut self, result: QueryResult, cache: Vec<Vec<String>>) {
        self.total_count = result.total_count;
        self.result = Some(result);
        self.display_cache = cache;
        self.is_loading = false;
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

    pub fn show(&mut self, ui: &mut egui::Ui, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
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

            // Pagination bar
            if self.result.is_some() {
                let total_pages = self
                    .total_count
                    .map(|tc| ((tc as f64) / (self.page_size as f64)).ceil() as usize)
                    .unwrap_or(0)
                    .max(1);
                let current = self.page + 1;
                let is_last = current >= total_pages;

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Next
                        let next = ui.add_enabled(
                            !is_last,
                            egui::Button::new(egui_phosphor::regular::CARET_RIGHT).small(),
                        );
                        if next.clicked() {
                            self.page += 1;
                            self.load(tab_id, cmd_tx);
                        }

                        // Page info: "1 / 5"
                        let page_label = if let Some(tc) = self.total_count {
                            format!("{current} / {total_pages}  ({tc} rows)")
                        } else {
                            format!("Page {current}")
                        };
                        ui.label(egui::RichText::new(page_label).color(hint_color).small());

                        // Prev
                        let prev = ui.add_enabled(
                            self.page > 0,
                            egui::Button::new(egui_phosphor::regular::CARET_LEFT).small(),
                        );
                        if prev.clicked() {
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
