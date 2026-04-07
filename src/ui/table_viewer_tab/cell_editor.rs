/// Cell editor popup — inline editing for a single cell value with JSON support.
use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::DbValue;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::TableViewerTab;

// ── Types ─────────────────────────────────────────────────────────────────────

pub(super) enum CellEditorAction {
    None,
    Save,
    Close,
}

pub(super) struct CellEditor {
    pub row: usize,
    #[allow(dead_code)]
    pub col: usize,
    pub column_name: String,
    pub original_value: String,
    pub edit_value: String,
    pub is_json: bool,
    pub json_error: Option<String>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Build a `CellEditor` from a row/col in the current result set.
pub(super) fn build_cell_editor(
    result: &suprim_sql::db::types::QueryResult,
    row: usize,
    col: usize,
) -> Option<CellEditor> {
    let col_meta = result.columns.get(col)?;
    let db_val = result.rows.get(row).and_then(|r| r.get(col));
    let (raw, is_json) = match db_val {
        Some(DbValue::Json(v)) => {
            let pretty = serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string());
            (pretty, true)
        }
        Some(v) => {
            let s = v.display();
            let looks_json = s.starts_with('{') || s.starts_with('[');
            if looks_json {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&s) {
                    let pretty = serde_json::to_string_pretty(&parsed).unwrap_or(s.clone());
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
    Some(CellEditor {
        row,
        col,
        column_name: col_meta.name.clone(),
        original_value: raw.clone(),
        edit_value: raw,
        is_json,
        json_error: None,
    })
}

// ── Impl on TableViewerTab ────────────────────────────────────────────────────

impl TableViewerTab {
    /// Render the cell-editor popup when active.
    pub(super) fn render_cell_editor_popup(
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
                    Self::render_editor_header(ui, &col_name, is_json);
                    ui.add_space(4.0);

                    let text_height = (ui.available_height() - 38.0).max(80.0);

                    if is_json {
                        Self::render_json_editor(ui, &mut editor.edit_value, text_height);
                    } else {
                        Self::render_plain_editor(ui, &mut editor.edit_value, text_height);
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
                    Self::render_editor_buttons(ui, editor, is_json, &mut action);
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

    // ── Private UI helpers ────────────────────────────────────────────────────

    fn render_editor_header(ui: &mut egui::Ui, col_name: &str, is_json: bool) {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Column: {col_name}"))
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
            if is_json {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new("JSON")
                            .small()
                            .color(egui::Color32::from_rgb(86, 156, 214)),
                    );
                });
            }
        });
    }

    fn render_json_editor(ui: &mut egui::Ui, edit_value: &mut String, text_height: f32) {
        use egui_code_editor::{CodeEditor, ColorTheme, Syntax};

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
                    .show(ui, edit_value);
            });
    }

    fn render_plain_editor(ui: &mut egui::Ui, edit_value: &mut String, text_height: f32) {
        egui::ScrollArea::vertical()
            .max_height(text_height)
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_sized(
                    [ui.available_width(), text_height],
                    egui::TextEdit::multiline(edit_value).font(egui::TextStyle::Monospace),
                );
            });
    }

    fn render_editor_buttons(
        ui: &mut egui::Ui,
        editor: &mut CellEditor,
        is_json: bool,
        action: &mut CellEditorAction,
    ) {
        ui.horizontal(|ui| {
            let changed = editor.edit_value != editor.original_value;
            if is_json {
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
                    match serde_json::from_str::<serde_json::Value>(&editor.edit_value) {
                        Ok(_) => {
                            editor.json_error = None;
                            *action = CellEditorAction::Save;
                        }
                        Err(e) => {
                            editor.json_error = Some(format!("Invalid JSON: {e}"));
                        }
                    }
                } else {
                    *action = CellEditorAction::Save;
                }
            }
            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                *action = CellEditorAction::Close;
            }
        });
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
            DbValue::Text(editor.edit_value.clone()),
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
        self.load(tab_id, cmd_tx);
    }
}
