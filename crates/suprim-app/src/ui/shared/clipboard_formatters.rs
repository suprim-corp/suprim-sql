/// Shared clipboard formatting utilities for DbValue → string conversion.
///
/// Eliminates duplication between sql_editor_tab and table_viewer_tab/cell_actions.
use suprim_core::db::types::DbValue;

/// Format a DbValue as a pretty-printed JSON string for clipboard.
pub fn format_as_json(val: &DbValue) -> String {
    match val {
        DbValue::Json(v) => serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
        DbValue::Null => "null".to_string(),
        other => serde_json::to_string(&other.display()).unwrap_or_default(),
    }
}

/// Format a DbValue as a CSV-safe string (quoted when necessary).
pub fn format_as_csv(val: &DbValue) -> String {
    let raw = val.display();
    if raw.contains(',') || raw.contains('"') || raw.contains('\n') {
        format!("\"{}\"", raw.replace('"', "\"\""))
    } else {
        raw
    }
}

/// Format a DbValue as a SQL literal suitable for an INSERT/UPDATE statement.
pub fn format_as_sql(val: &DbValue) -> String {
    match val {
        DbValue::Null => "NULL".to_string(),
        DbValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        DbValue::Int(i) => i.to_string(),
        DbValue::Float(f) => f.to_string(),
        DbValue::Decimal(s) => s.clone(),
        DbValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
        DbValue::Json(v) => {
            format!("'{}'::jsonb", v.to_string().replace('\'', "''"))
        }
        DbValue::Bytes(b) => {
            let hex_str: String = b.iter().map(|byte| format!("{:02x}", byte)).collect();
            format!("'\\x{}'", hex_str)
        }
        DbValue::Timestamp(t) => {
            format!("'{}'", t.format("%Y-%m-%d %H:%M:%S"))
        }
    }
}
