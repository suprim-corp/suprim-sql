//! SQL writer — INSERT statements with optional DROP/CREATE.

use std::io::Write;
use std::path::Path;

use suprim_core::db::dialect::SqlDialect;
use suprim_core::db::values::{DbValue, QueryResult};
use suprim_core::db::TableNode;

use super::super::sql_options::SqlOptions;

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
    /// SQL dialect — determines quoting style and literal formatting.
    pub dialect: SqlDialect,
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
    let dialect = tbl.dialect;
    let qualified = dialect.quote_table(tbl.schema, tbl.name);

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
                suprim_core::db::ddl_generator::full_table_ddl(tbl.schema, node, dialect)
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
                    format!("    {} {}", dialect.quote_ident(&c.name), ty)
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
    let dialect = tbl.dialect;
    let qualified = dialect.quote_table(tbl.schema, tbl.name);
    let col_list: Vec<String> = tbl
        .result
        .columns
        .iter()
        .map(|c| dialect.quote_ident(&c.name))
        .collect();
    let col_list = col_list.join(", ");

    let batch = opts.batch_size.value();
    for chunk in tbl.result.rows.chunks(batch) {
        writeln!(f, "INSERT INTO {qualified} ({col_list}) VALUES")?;
        let last = chunk.len() - 1;
        for (i, row) in chunk.iter().enumerate() {
            let vals: Vec<String> = row.iter().map(|v| sql_literal(v, dialect)).collect();
            let terminator = if i == last { ";" } else { "," };
            writeln!(f, "  ({}){terminator}", vals.join(", "))?;
        }
    }

    Ok(())
}

/// Render a DbValue as a SQL literal suitable for an INSERT.
fn sql_literal(val: &DbValue, dialect: SqlDialect) -> String {
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
        DbValue::Decimal(s) => s.clone(),
        DbValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
        DbValue::Json(v) => dialect.json_literal(&v.to_string()),
        DbValue::Bytes(b) => dialect.bytes_literal(b),
        DbValue::Timestamp(t) => format!("'{}'", t.format("%Y-%m-%d %H:%M:%S")),
    }
}
