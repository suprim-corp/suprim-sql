/// Shared clipboard formatting utilities for DbValue → string conversion.
///
/// Eliminates duplication between sql_editor_tab and table_viewer_tab/cell_actions.
use suprim_core::db::dialect::SqlDialect;
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
pub fn format_as_sql(val: &DbValue, dialect: SqlDialect) -> String {
    match val {
        DbValue::Null => "NULL".to_string(),
        DbValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        DbValue::Int(i) => i.to_string(),
        DbValue::Float(f) => f.to_string(),
        DbValue::Decimal(s) => s.clone(),
        DbValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
        DbValue::Json(v) => dialect.json_literal(&v.to_string()),
        DbValue::Bytes(b) => dialect.bytes_literal(b),
        DbValue::Timestamp(t) => {
            format!("'{}'", t.format("%Y-%m-%d %H:%M:%S"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_as_sql_postgres_json() {
        let val = DbValue::Json(serde_json::json!({"key": "value"}));
        let result = format_as_sql(&val, SqlDialect::Postgres);
        assert!(
            result.contains("::jsonb"),
            "PG JSON should use ::jsonb cast: {result}"
        );
    }

    #[test]
    fn format_as_sql_mysql_json() {
        let val = DbValue::Json(serde_json::json!({"key": "value"}));
        let result = format_as_sql(&val, SqlDialect::Mysql);
        assert!(
            result.contains("CAST("),
            "MySQL JSON should use CAST: {result}"
        );
        assert!(
            result.contains("AS JSON"),
            "MySQL JSON should cast AS JSON: {result}"
        );
        assert!(
            !result.contains("::jsonb"),
            "MySQL should NOT use ::jsonb: {result}"
        );
    }

    #[test]
    fn format_as_sql_postgres_bytes() {
        let val = DbValue::Bytes(vec![0xde, 0xad]);
        let result = format_as_sql(&val, SqlDialect::Postgres);
        assert_eq!(result, "'\\xdead'");
    }

    #[test]
    fn format_as_sql_mysql_bytes() {
        let val = DbValue::Bytes(vec![0xde, 0xad]);
        let result = format_as_sql(&val, SqlDialect::Mysql);
        assert_eq!(result, "X'dead'");
    }

    #[test]
    fn format_as_sql_null() {
        assert_eq!(format_as_sql(&DbValue::Null, SqlDialect::Postgres), "NULL");
        assert_eq!(format_as_sql(&DbValue::Null, SqlDialect::Mysql), "NULL");
    }

    #[test]
    fn format_as_sql_text_escapes_quotes() {
        let val = DbValue::Text("it's a test".to_string());
        let result = format_as_sql(&val, SqlDialect::Postgres);
        assert_eq!(result, "'it''s a test'");
    }

    #[test]
    fn format_as_sql_decimal_no_quotes() {
        let val = DbValue::Decimal("12345.67".to_string());
        let result = format_as_sql(&val, SqlDialect::Mysql);
        assert_eq!(result, "12345.67", "Decimal should not be quoted");
    }
}
