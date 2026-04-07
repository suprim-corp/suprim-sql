/// Table structure editor tab — view and edit columns, indexes, and foreign keys.
mod columns_grid;
mod detail_sections;
mod sql_generator;

use eframe::egui;
use suprim_sql::db::driver::DbCommand;
use suprim_sql::db::types::{ForeignKeyNode, IndexNode, TableNode};
use tokio::sync::mpsc;
use uuid::Uuid;

/// Editable copy of a column row.
#[derive(Clone)]
pub(crate) struct EditableColumn {
    pub name: String,
    pub db_type: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub default_value: String,
    /// `true` if this column was in the original table (vs. newly added).
    pub original: bool,
}

impl From<&suprim_sql::db::types::ColumnNode> for EditableColumn {
    fn from(c: &suprim_sql::db::types::ColumnNode) -> Self {
        Self {
            name: c.name.clone(),
            db_type: c.db_type.clone(),
            nullable: c.nullable,
            is_primary_key: c.is_primary_key,
            default_value: c.default_value.clone().unwrap_or_default(),
            original: true,
        }
    }
}

pub struct TableEditorTab {
    pub conn_id: Uuid,
    pub database: String,
    pub schema_name: String,
    pub table_name: String,

    columns: Vec<EditableColumn>,
    indexes: Vec<IndexNode>,
    foreign_keys: Vec<ForeignKeyNode>,

    /// Generated SQL preview (filled when user clicks "Preview SQL").
    sql_preview: String,
    show_sql_preview: bool,
    /// Status message after save attempt.
    status_message: Option<String>,
}

impl TableEditorTab {
    pub fn new(conn_id: Uuid, database: String, schema_name: String, table: &TableNode) -> Self {
        Self {
            conn_id,
            database,
            schema_name,
            table_name: table.name.clone(),
            columns: table.columns.iter().map(EditableColumn::from).collect(),
            indexes: table.indexes.clone(),
            foreign_keys: table.foreign_keys.clone(),
            sql_preview: String::new(),
            show_sql_preview: false,
            status_message: None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        ui.vertical(|ui| {
            // ── Header ──────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.heading(format!(
                    "{} Edit: {}.{}",
                    egui_phosphor::regular::PENCIL_SIMPLE,
                    self.schema_name,
                    self.table_name,
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Save Changes").clicked() {
                        let sql = sql_generator::generate_alter_sql(
                            &self.schema_name,
                            &self.table_name,
                            &self.columns,
                        );
                        self.status_message = Some(sql_generator::execute_changes(
                            self.conn_id,
                            tab_id,
                            &sql,
                            &mut self.columns,
                            cmd_tx,
                        ));
                    }
                    if ui.button("Preview SQL").clicked() {
                        self.sql_preview = sql_generator::generate_alter_sql(
                            &self.schema_name,
                            &self.table_name,
                            &self.columns,
                        );
                        self.show_sql_preview = true;
                    }
                    if ui.button("Add Column").clicked() {
                        self.columns.push(EditableColumn {
                            name: String::new(),
                            db_type: "text".to_string(),
                            nullable: true,
                            is_primary_key: false,
                            default_value: String::new(),
                            original: false,
                        });
                    }
                });
            });
            ui.separator();

            if let Some(msg) = &self.status_message {
                ui.colored_label(ui.visuals().warn_fg_color, msg);
                ui.add_space(4.0);
            }

            // ── Scrollable content ──────────────────────────────────
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    columns_grid::render_columns_grid(&mut self.columns, ui);

                    ui.add_space(16.0);
                    detail_sections::render_indexes_section(&self.indexes, ui);

                    ui.add_space(16.0);
                    detail_sections::render_foreign_keys_section(&self.foreign_keys, ui);

                    // ── SQL Preview ─────────────────────────────────
                    if self.show_sql_preview && !self.sql_preview.is_empty() {
                        ui.add_space(16.0);
                        ui.separator();
                        ui.label(egui::RichText::new("SQL Preview").strong().size(14.0));
                        ui.add_space(4.0);

                        let mut preview = self.sql_preview.clone();
                        ui.add(
                            egui::TextEdit::multiline(&mut preview)
                                .code_editor()
                                .desired_rows(6)
                                .desired_width(f32::INFINITY),
                        );
                    }
                });
        });
    }
}
