//! CSV writer — export a `QueryResult` to CSV with configurable options.

use std::io::Write;
use std::path::Path;

use suprim_core::db::values::{DbValue, QueryResult};

use super::super::csv_options::CsvOptions;

/// Export a single `QueryResult` to CSV with the given options.
pub fn export(result: &QueryResult, path: &Path, opts: &CsvOptions) -> std::io::Result<()> {
    let mut f = super::create_writer(path, opts.gzip)?;
    let sep = opts.delimiter.char();
    let lb = opts.line_break.value();

    // Header row
    if opts.include_field_names {
        let headers: Vec<String> = result
            .columns
            .iter()
            .map(|c| format_cell(&c.name, sep, opts))
            .collect();
        write!(f, "{}{}", headers.join(&sep.to_string()), lb)?;
    }

    // Data rows
    for row in &result.rows {
        let cells: Vec<String> = row.iter().map(|v| format_db_value(v, sep, opts)).collect();
        write!(f, "{}{}", cells.join(&sep.to_string()), lb)?;
    }

    Ok(())
}

/// Format a `DbValue` as a CSV cell string.
fn format_db_value(val: &DbValue, sep: char, opts: &CsvOptions) -> String {
    match val {
        DbValue::Null => {
            if opts.convert_null_to_empty {
                String::new()
            } else {
                "NULL".to_string()
            }
        }
        DbValue::Bool(b) => b.to_string(),
        DbValue::Int(i) => i.to_string(),
        DbValue::Float(f) => f.to_string(),
        DbValue::Decimal(s) => s.clone(),
        DbValue::Timestamp(t) => format_cell(&t.format("%Y-%m-%d %H:%M:%S").to_string(), sep, opts),
        DbValue::Bytes(b) => {
            let hex: String = b.iter().map(|byte| format!("{:02x}", byte)).collect();
            format_cell(&format!("\\x{hex}"), sep, opts)
        }
        DbValue::Text(s) => format_cell(s, sep, opts),
        DbValue::Json(v) => format_cell(&v.to_string(), sep, opts),
    }
}

/// Apply quote + escape rules + formula sanitation + line-break conversion to a string cell.
fn format_cell(raw: &str, sep: char, opts: &CsvOptions) -> String {
    let mut s = raw.to_string();

    // Convert line breaks to spaces if requested.
    if opts.convert_line_break_to_space {
        s = s.replace(['\n', '\r'], " ");
    }

    // Sanitize formula-like values (prevent CSV injection).
    if opts.sanitize_formulas {
        if let Some(first) = s.chars().next() {
            if matches!(first, '=' | '+' | '-' | '@' | '\t' | '\r') {
                s = format!("'{s}");
            }
        }
    }

    // Quote handling
    let needs_quote = s.contains(sep) || s.contains('"') || s.contains('\n') || s.contains('\r');
    match opts.quote_handling {
        super::super::csv_options::QuoteHandling::Never => s,
        super::super::csv_options::QuoteHandling::Always => {
            format!("\"{}\"", s.replace('"', "\"\""))
        }
        super::super::csv_options::QuoteHandling::AsNeeded => {
            if needs_quote {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s
            }
        }
    }
}
