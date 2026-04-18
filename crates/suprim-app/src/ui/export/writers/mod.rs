//! Export writers — CSV, JSON, SQL file writers + shared dispatch.

pub mod csv;
pub mod json;
pub mod sql;

use std::path::Path;

use suprim_core::db::dialect::SqlDialect;
use suprim_core::db::values::QueryResult;

use super::types::{FormatOptions, SqlExportInfo};

/// Create a writer for the given path. When `gzip` is true, wraps in a
/// `GzEncoder` for on-the-fly compression.
pub(crate) fn create_writer(path: &Path, gzip: bool) -> std::io::Result<Box<dyn std::io::Write>> {
    let file = std::fs::File::create(path)?;
    let buf = std::io::BufWriter::new(file);
    if gzip {
        Ok(Box::new(flate2::write::GzEncoder::new(
            buf,
            flate2::Compression::default(),
        )))
    } else {
        Ok(Box::new(buf))
    }
}

/// Dispatch an export to the correct writer based on `FormatOptions`.
///
/// For `Sql` format, `sql_export_info` provides per-table metadata (schema name,
/// table name, DDL toggles, table node). When `None`, defaults are used suitable
/// for a query-result export.
pub fn execute_export(
    result: &QueryResult,
    path: &Path,
    format: &FormatOptions,
    sql_export_info: Option<SqlExportInfo<'_>>,
) -> std::io::Result<()> {
    match format {
        FormatOptions::Csv(opts) => csv::export(result, path, opts),
        FormatOptions::Json(opts) => json::export(result, path, opts),
        FormatOptions::Sql(opts) => {
            let info = sql_export_info.unwrap_or(SqlExportInfo {
                schema: "public",
                table_name: "query_result",
                include_structure: true,
                include_drop: false,
                include_data: true,
                table_node: None,
                dialect: SqlDialect::default(),
            });
            let tbl = sql::SqlTableExport {
                schema: info.schema,
                name: info.table_name,
                result,
                include_structure: info.include_structure,
                include_drop: info.include_drop,
                include_data: info.include_data,
                table_node: info.table_node,
                dialect: info.dialect,
            };
            sql::export(&[tbl], path, opts)
        }
    }
}
