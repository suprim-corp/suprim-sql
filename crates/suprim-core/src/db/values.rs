/// Universal value types — DbValue, ColumnMeta, QueryResult.
/// All DB drivers map native types to these; the UI layer only works with them.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Universal value type — all DB drivers map native types to DbValue.
/// The UI layer only ever works with DbValue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DbValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
    Json(serde_json::Value),
    Timestamp(DateTime<Utc>),
}

impl DbValue {
    pub fn is_null(&self) -> bool {
        matches!(self, DbValue::Null)
    }

    /// Display string for the UI table renderer
    pub fn display(&self) -> String {
        match self {
            DbValue::Null => "NULL".to_string(),
            DbValue::Bool(b) => b.to_string(),
            DbValue::Int(i) => i.to_string(),
            DbValue::Float(f) => f.to_string(),
            DbValue::Text(s) => s.clone(),
            DbValue::Bytes(b) => format!("<{} bytes>", b.len()),
            DbValue::Json(v) => v.to_string(),
            DbValue::Timestamp(t) => t.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        }
    }
}

impl std::fmt::Display for DbValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}

/// Metadata for a single column in a query result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnMeta {
    pub name: String,
    /// Raw type string from the database (e.g. "int4", "varchar", "DATETIME")
    pub db_type: String,
    pub nullable: bool,
}

/// Result of a query execution
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub columns: Vec<ColumnMeta>,
    pub rows: Vec<Vec<DbValue>>,
    /// Rows affected — relevant for INSERT/UPDATE/DELETE
    pub rows_affected: u64,
    pub execution_time: Duration,
    /// Total row count (before LIMIT) — used for pagination display.
    /// Only set by `table_data` queries, `None` for raw SQL execution.
    pub total_count: Option<u64>,
}

impl QueryResult {
    pub fn empty() -> Self {
        Self {
            columns: vec![],
            rows: vec![],
            rows_affected: 0,
            execution_time: Duration::ZERO,
            total_count: None,
        }
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn db_value_display_null() {
        assert_eq!(DbValue::Null.display(), "NULL");
    }

    #[test]
    fn db_value_display_bool() {
        assert_eq!(DbValue::Bool(true).display(), "true");
        assert_eq!(DbValue::Bool(false).display(), "false");
    }

    #[test]
    fn db_value_display_int() {
        assert_eq!(DbValue::Int(42).display(), "42");
        assert_eq!(DbValue::Int(-1).display(), "-1");
    }

    #[test]
    fn db_value_display_float() {
        assert_eq!(DbValue::Float(3.14).display(), "3.14");
    }

    #[test]
    fn db_value_display_text() {
        assert_eq!(DbValue::Text("hello".into()).display(), "hello");
    }

    #[test]
    fn db_value_display_bytes() {
        assert_eq!(DbValue::Bytes(vec![1, 2, 3]).display(), "<3 bytes>");
    }

    #[test]
    fn db_value_display_json() {
        let v = DbValue::Json(json!({"key": "value"}));
        assert!(v.display().contains("key"));
    }

    #[test]
    fn db_value_is_null() {
        assert!(DbValue::Null.is_null());
        assert!(!DbValue::Int(0).is_null());
    }

    #[test]
    fn db_value_serde_roundtrip() {
        let values = vec![
            DbValue::Null,
            DbValue::Bool(true),
            DbValue::Int(99),
            DbValue::Float(1.5),
            DbValue::Text("test".into()),
            DbValue::Json(json!(null)),
        ];
        for v in values {
            let serialized = serde_json::to_string(&v).unwrap();
            let deserialized: DbValue = serde_json::from_str(&serialized).unwrap();
            assert_eq!(v, deserialized);
        }
    }

    #[test]
    fn query_result_empty() {
        let r = QueryResult::empty();
        assert_eq!(r.row_count(), 0);
        assert_eq!(r.column_count(), 0);
        assert_eq!(r.rows_affected, 0);
    }

    #[test]
    fn db_value_display_timestamp() {
        use chrono::TimeZone;
        let ts = chrono::Utc.with_ymd_and_hms(2024, 1, 15, 12, 0, 0).unwrap();
        let v = DbValue::Timestamp(ts);
        assert!(v.display().contains("2024"));
    }

    #[test]
    fn db_value_display_fmt() {
        let v = DbValue::Int(7);
        assert_eq!(format!("{}", v), "7");
    }
}
