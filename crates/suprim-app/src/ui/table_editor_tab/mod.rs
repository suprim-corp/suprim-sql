/// Table structure editor tab — view and edit columns, indexes, and foreign keys.
mod columns_grid;
pub(crate) mod default_suggestions;
mod detail_sections;
mod sql_generator;

use eframe::egui;
use suprim_core::db::commands::DbCommand;
use suprim_core::db::dialect::SqlDialect;
use suprim_core::db::types::{ForeignKeyNode, IndexNode, TableNode};
use tokio::sync::mpsc;
use uuid::Uuid;

/// Editable copy of a column row.
#[derive(Clone)]
pub(crate) struct EditableColumn {
    pub name: String,
    /// Base type name (e.g. `varchar`, `numeric`).
    pub db_type: String,
    /// Optional length/precision parameter (e.g. `255`, `10,2`).
    /// Combined with `db_type` when generating DDL: `varchar(255)`.
    pub type_param: String,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub default_value: String,
    /// `true` if this column was in the original table (vs. newly added).
    pub original: bool,
}

impl From<&suprim_core::db::types::ColumnNode> for EditableColumn {
    fn from(c: &suprim_core::db::types::ColumnNode) -> Self {
        // Parse "varchar(255)" → base="varchar", param="255"
        let (base, param) = parse_type_and_param(&c.db_type);
        Self {
            name: c.name.clone(),
            db_type: base,
            type_param: param,
            nullable: c.nullable,
            is_primary_key: c.is_primary_key,
            default_value: c.default_value.clone().unwrap_or_default(),
            original: true,
        }
    }
}

/// Split a full type string like `varchar(255)` or `numeric(10,2)` into
/// `("varchar", "255")` or `("numeric", "10,2")`. Types without params
/// return an empty param string.
fn parse_type_and_param(full_type: &str) -> (String, String) {
    if let Some(open) = full_type.find('(') {
        let base = full_type[..open].trim().to_string();
        let rest = &full_type[open + 1..];
        let param = rest.trim_end_matches(')').trim().to_string();
        (base, param)
    } else {
        (full_type.to_string(), String::new())
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
    /// `true` when creating a brand-new table (vs. editing an existing one).
    pub is_new_table: bool,
    /// Function signatures available in the current schema (for default value autocomplete).
    pub schema_functions: Vec<String>,
    /// SQL dialect for this tab's connection (affects quoting/literal formatting).
    pub dialect: SqlDialect,
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
            is_new_table: false,
            schema_functions: Vec::new(),
            dialect: SqlDialect::default(),
        }
    }

    /// Create an empty editor for designing a brand-new table.
    pub fn new_empty(conn_id: Uuid, database: String, schema_name: String) -> Self {
        Self {
            conn_id,
            database,
            schema_name,
            table_name: String::new(),
            columns: vec![EditableColumn {
                name: "id".to_string(),
                db_type: "bigint".to_string(),
                type_param: String::new(),
                nullable: false,
                is_primary_key: true,
                default_value: String::new(),
                original: false,
            }],
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
            sql_preview: String::new(),
            show_sql_preview: false,
            status_message: None,
            is_new_table: true,
            schema_functions: Vec::new(),
            dialect: SqlDialect::default(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, tab_id: Uuid, cmd_tx: &mpsc::Sender<DbCommand>) {
        ui.vertical(|ui| {
            // ── Header ──────────────────────────────────────────────
            ui.horizontal(|ui| {
                if self.is_new_table {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}  New Table in {}.{}",
                            egui_phosphor::regular::PLUS_CIRCLE,
                            self.database,
                            self.schema_name,
                        ))
                        .heading(),
                    );
                } else {
                    ui.heading(format!(
                        "{} Edit: {}.{}",
                        egui_phosphor::regular::PENCIL_SIMPLE,
                        self.schema_name,
                        self.table_name,
                    ));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let save_label = if self.is_new_table {
                        "Create Table"
                    } else {
                        "Save Changes"
                    };
                    if ui
                        .button(save_label)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        if self.is_new_table {
                            let sql = sql_generator::generate_create_table_sql(
                                &self.schema_name,
                                &self.table_name,
                                &self.columns,
                                self.dialect,
                            );
                            self.status_message = Some(sql_generator::execute_changes(
                                self.conn_id,
                                tab_id,
                                &self.database,
                                &sql,
                                &mut self.columns,
                                cmd_tx,
                            ));
                        } else {
                            let sql = sql_generator::generate_add_columns_sql(
                                &self.schema_name,
                                &self.table_name,
                                &self.columns,
                                self.dialect,
                            );
                            self.status_message = Some(sql_generator::execute_changes(
                                self.conn_id,
                                tab_id,
                                &self.database,
                                &sql,
                                &mut self.columns,
                                cmd_tx,
                            ));
                        }
                    }
                    if ui
                        .button("Preview SQL")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.sql_preview = if self.is_new_table {
                            sql_generator::generate_create_table_sql(
                                &self.schema_name,
                                &self.table_name,
                                &self.columns,
                                self.dialect,
                            )
                        } else {
                            sql_generator::generate_add_columns_sql(
                                &self.schema_name,
                                &self.table_name,
                                &self.columns,
                                self.dialect,
                            )
                        };
                        self.show_sql_preview = true;
                    }
                    if ui
                        .button("Add Column")
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        self.columns.push(EditableColumn {
                            name: String::new(),
                            db_type: "text".to_string(),
                            type_param: String::new(),
                            nullable: true,
                            is_primary_key: false,
                            default_value: String::new(),
                            original: false,
                        });
                    }
                });
            });
            ui.separator();

            // ── Table name input (new table mode only) ──────────────
            if self.is_new_table {
                ui.horizontal(|ui| {
                    ui.label("Table name:");
                    ui.text_edit_singleline(&mut self.table_name);
                });
                ui.add_space(4.0);
            }

            if let Some(msg) = &self.status_message {
                ui.colored_label(ui.visuals().warn_fg_color, msg);
                ui.add_space(4.0);
            }

            // ── Scrollable content ──────────────────────────────────
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    columns_grid::render_columns_grid(
                        &mut self.columns,
                        &self.schema_functions,
                        self.dialect,
                        ui,
                    );

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
