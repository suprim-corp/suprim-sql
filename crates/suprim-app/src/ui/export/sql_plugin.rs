//! SQL export plugin — INSERT statements with optional DROP/CREATE.
//!
//! Per-table toggles (Structure, Drop, Data) live on `ExportTableItem`.
//! Global options live here (batch size, gzip).

use std::io::Write;
use std::path::Path;

use eframe::egui;

use suprim_core::db::values::{DbValue, QueryResult};
use suprim_core::db::TableNode;

// ── Options ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchSize {
    One,
    Hundred,
    FiveHundred,
    Thousand,
}

impl BatchSize {
    pub fn label(&self) -> &'static str {
        match self {
            BatchSize::One => "1 (one INSERT per row)",
            BatchSize::Hundred => "100",
            BatchSize::FiveHundred => "500",
            BatchSize::Thousand => "1000",
        }
    }
    pub fn value(&self) -> usize {
        match self {
            BatchSize::One => 1,
            BatchSize::Hundred => 100,
            BatchSize::FiveHundred => 500,
            BatchSize::Thousand => 1000,
        }
    }
    pub fn all() -> &'static [BatchSize] {
        &[
            BatchSize::One,
            BatchSize::Hundred,
            BatchSize::FiveHundred,
            BatchSize::Thousand,
        ]
    }
}

#[derive(Debug, Clone)]
pub struct SqlOptions {
    pub batch_size: BatchSize,
    pub gzip: bool,
}

impl Default for SqlOptions {
    fn default() -> Self {
        Self {
            batch_size: BatchSize::FiveHundred,
            gzip: false,
        }
    }
}

// ── UI ──────────────────────────────────────────────────────────────────────

pub fn render_options_ui(ui: &mut egui::Ui, opts: &mut SqlOptions) {
    ui.label(
        egui::RichText::new(
            "Structure, Drop, and Data options are configured per table in the table list.",
        )
        .weak()
        .size(11.0),
    );
    ui.add_space(10.0);

    egui::Grid::new("sql_dropdowns")
        .num_columns(2)
        .spacing([12.0, 8.0])
        .show(ui, |ui| {
            ui.label("Rows per INSERT");
            egui::ComboBox::from_id_salt("sql_batch_size")
                .selected_text(opts.batch_size.label())
                .width(200.0)
                .show_ui(ui, |ui| {
                    for b in BatchSize::all() {
                        if ui
                            .selectable_label(opts.batch_size == *b, b.label())
                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                            .clicked()
                        {
                            opts.batch_size = *b;
                        }
                    }
                });
            ui.end_row();
        });

    ui.add_space(4.0);
    ui.checkbox(&mut opts.gzip, "Compress the file using Gzip");
}

// ── Writer ──────────────────────────────────────────────────────────────────

/// Information the writer needs for a single table export.
pub struct SqlTableExport<'a> {
    pub schema: &'a str,
    pub name: &'a str,
    pub result: &'a QueryResult,
    pub include_structure: bool,
    pub include_drop: bool,
    pub include_data: bool,
    /// Full table metadata from the sidebar (columns, indexes, FKs).
    /// When present, DDL is generated from real metadata instead of a skeleton.
    pub table_node: Option<&'a TableNode>,
}

/// Export tables (one or many) to a single SQL file.
pub fn export(
    tables: &[SqlTableExport<'_>],
    path: &Path,
    opts: &SqlOptions,
) -> std::io::Result<()> {
    let mut f = super::create_writer(path, opts.gzip)?;

    // File header
    writeln!(
        f,
        "-- SuprimSQL SQL export\n-- Generated at {}\n",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )?;

    for tbl in tables {
        write_table(&mut f, tbl, opts)?;
        writeln!(f)?;
    }
    Ok(())
}

fn write_table(
    f: &mut dyn Write,
    tbl: &SqlTableExport<'_>,
    opts: &SqlOptions,
) -> std::io::Result<()> {
    let qualified = format!("\"{}\".\"{}\"", tbl.schema, tbl.name);

    writeln!(f, "-- ── Table: {qualified} ──")?;

    if tbl.include_drop {
        writeln!(f, "DROP TABLE IF EXISTS {qualified};")?;
    }

    if tbl.include_structure {
        if let Some(node) = tbl.table_node {
            // Real DDL from schema metadata (includes indexes, FKs)
            writeln!(
                f,
                "{}",
                suprim_core::db::ddl_generator::full_table_ddl(tbl.schema, node)
            )?;
        } else {
            // Fallback: columns-only skeleton from query result metadata
            writeln!(
                f,
                "-- Note: full DDL not available (exported from query result)."
            )?;
            writeln!(f, "CREATE TABLE IF NOT EXISTS {qualified} (")?;
            let cols: Vec<String> = tbl
                .result
                .columns
                .iter()
                .map(|c| {
                    let ty = if c.db_type.is_empty() {
                        "text".to_string()
                    } else {
                        c.db_type.clone()
                    };
                    format!("    \"{}\" {}", c.name, ty)
                })
                .collect();
            writeln!(f, "{}", cols.join(",\n"))?;
            writeln!(f, ");")?;
        }
    }

    if tbl.include_data && !tbl.result.rows.is_empty() {
        write_insert_statements(f, tbl, opts)?;
    }

    Ok(())
}

fn write_insert_statements(
    f: &mut dyn Write,
    tbl: &SqlTableExport<'_>,
    opts: &SqlOptions,
) -> std::io::Result<()> {
    let qualified = format!("\"{}\".\"{}\"", tbl.schema, tbl.name);
    let col_list: Vec<String> = tbl
        .result
        .columns
        .iter()
        .map(|c| format!("\"{}\"", c.name))
        .collect();
    let col_list = col_list.join(", ");

    let batch = opts.batch_size.value();
    for chunk in tbl.result.rows.chunks(batch) {
        writeln!(f, "INSERT INTO {qualified} ({col_list}) VALUES")?;
        let last = chunk.len() - 1;
        for (i, row) in chunk.iter().enumerate() {
            let vals: Vec<String> = row.iter().map(sql_literal).collect();
            let terminator = if i == last { ";" } else { "," };
            writeln!(f, "  ({}){terminator}", vals.join(", "))?;
        }
    }

    Ok(())
}

/// Render a DbValue as a SQL literal suitable for an INSERT.
fn sql_literal(val: &DbValue) -> String {
    match val {
        DbValue::Null => "NULL".to_string(),
        DbValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        DbValue::Int(i) => i.to_string(),
        DbValue::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                "NULL".to_string()
            } else {
                f.to_string()
            }
        }
        DbValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
        DbValue::Json(v) => {
            let raw = v.to_string().replace('\'', "''");
            format!("'{raw}'::jsonb")
        }
        DbValue::Bytes(b) => {
            let hex: String = b.iter().map(|byte| format!("{:02x}", byte)).collect();
            format!("'\\x{hex}'")
        }
        DbValue::Timestamp(t) => format!("'{}'", t.format("%Y-%m-%d %H:%M:%S")),
    }
}
