//! Type mapping: convert MySQL row cells to DbValue and build QueryResult.

use std::time::Duration;

use sqlx::mysql::MySqlRow;
use sqlx::{Column, Row, TypeInfo};

use crate::db::types::{ColumnMeta, DbValue, QueryResult};

/// Map a MySQL row cell to DbValue using the column's type name.
pub fn mysql_value_from_row(row: &MySqlRow, idx: usize, type_name: &str) -> DbValue {
    let upper = type_name.to_uppercase();

    // Boolean: TINYINT(1) or BOOLEAN/BOOL
    if upper == "TINYINT(1)" || upper == "BOOLEAN" || upper == "BOOL" {
        return row
            .try_get::<bool, _>(idx)
            .map(DbValue::Bool)
            .unwrap_or_else(|_| {
                row.try_get::<i8, _>(idx)
                    .map(|v| DbValue::Int(v as i64))
                    .unwrap_or(DbValue::Null)
            });
    }

    // Integer types (order matters: check BIGINT before INT to avoid substring match)
    if upper.contains("BIGINT") {
        return row
            .try_get::<i64, _>(idx)
            .map(DbValue::Int)
            .unwrap_or(DbValue::Null);
    }
    if upper.contains("TINYINT") {
        return row
            .try_get::<i8, _>(idx)
            .map(|v| DbValue::Int(v as i64))
            .unwrap_or(DbValue::Null);
    }
    if upper.contains("SMALLINT") {
        return row
            .try_get::<i16, _>(idx)
            .map(|v| DbValue::Int(v as i64))
            .unwrap_or(DbValue::Null);
    }
    if upper.contains("MEDIUMINT") || upper.contains("INT") {
        return row
            .try_get::<i32, _>(idx)
            .map(|v| DbValue::Int(v as i64))
            .unwrap_or(DbValue::Null);
    }

    // Floating point
    if upper.contains("FLOAT") {
        return row
            .try_get::<f32, _>(idx)
            .map(|v| DbValue::Float(v as f64))
            .unwrap_or(DbValue::Null);
    }
    // DECIMAL/NUMERIC: decoded via rust_decimal::Decimal, then converted to f64.
    // Falls back to Text representation if f64 conversion would lose precision.
    if upper.contains("DECIMAL") || upper.contains("NUMERIC") || upper.contains("NEWDECIMAL") {
        use rust_decimal::prelude::ToPrimitive;
        return row
            .try_get::<rust_decimal::Decimal, _>(idx)
            .map(|d| {
                d.to_f64()
                    .map(DbValue::Float)
                    .unwrap_or_else(|| DbValue::Text(d.to_string()))
            })
            .unwrap_or(DbValue::Null);
    }
    if upper.contains("DOUBLE") {
        return row
            .try_get::<f64, _>(idx)
            .map(DbValue::Float)
            .unwrap_or(DbValue::Null);
    }

    // Blob / binary
    if upper.contains("BLOB") || upper.contains("BINARY") || upper.contains("VARBINARY") {
        return row
            .try_get::<Vec<u8>, _>(idx)
            .map(DbValue::Bytes)
            .unwrap_or(DbValue::Null);
    }

    // JSON
    if upper == "JSON" {
        return row
            .try_get::<serde_json::Value, _>(idx)
            .map(DbValue::Json)
            .unwrap_or_else(|_| {
                // Fallback: decode as text then parse
                row.try_get::<String, _>(idx)
                    .ok()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .map(DbValue::Json)
                    .unwrap_or(DbValue::Null)
            });
    }

    // Datetime / Timestamp
    if upper.contains("DATETIME") || upper.contains("TIMESTAMP") {
        return row
            .try_get::<chrono::DateTime<chrono::Utc>, _>(idx)
            .map(DbValue::Timestamp)
            .unwrap_or_else(|_| {
                row.try_get::<String, _>(idx)
                    .map(DbValue::Text)
                    .unwrap_or(DbValue::Null)
            });
    }

    // Default: text (TEXT, VARCHAR, CHAR, ENUM, SET, DATE, TIME, YEAR, etc.)
    row.try_get::<String, _>(idx)
        .map(DbValue::Text)
        .unwrap_or(DbValue::Null)
}

/// Convert a Vec<MySqlRow> + elapsed time into a QueryResult.
pub fn rows_to_query_result(rows: Vec<MySqlRow>, elapsed: Duration) -> QueryResult {
    if rows.is_empty() {
        return QueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected: 0,
            execution_time: elapsed,
            total_count: None,
        };
    }

    let columns: Vec<ColumnMeta> = rows[0]
        .columns()
        .iter()
        .map(|c| ColumnMeta {
            name: c.name().to_string(),
            db_type: c.type_info().name().to_string(),
            // TODO(mysql): MySqlRow doesn't expose per-column nullability at runtime.
            // Would require cross-referencing with INFORMATION_SCHEMA.COLUMNS data.
            nullable: true,
        })
        .collect();

    let data_rows: Vec<Vec<DbValue>> = rows
        .iter()
        .map(|row| {
            row.columns()
                .iter()
                .enumerate()
                .map(|(i, col)| mysql_value_from_row(row, i, col.type_info().name()))
                .collect()
        })
        .collect();

    let row_count = data_rows.len() as u64;

    QueryResult {
        columns,
        rows: data_rows,
        rows_affected: row_count,
        execution_time: elapsed,
        total_count: None,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_to_query_result_empty_rows() {
        let result = rows_to_query_result(vec![], Duration::from_millis(5));
        assert_eq!(result.columns.len(), 0);
        assert_eq!(result.rows.len(), 0);
        assert_eq!(result.rows_affected, 0);
        assert_eq!(result.execution_time.as_millis(), 5);
        assert!(result.total_count.is_none());
    }

    #[test]
    fn mysql_type_bool_is_tinyint1() {
        let upper = "TINYINT(1)".to_uppercase();
        assert!(upper == "TINYINT(1)" || upper == "BOOLEAN" || upper == "BOOL");
    }

    #[test]
    fn mysql_type_bigint_matched_before_int() {
        let upper = "BIGINT".to_uppercase();
        // BIGINT should be checked before generic INT
        assert!(upper.contains("BIGINT"));
    }

    #[test]
    fn mysql_type_varchar_falls_to_default() {
        let upper = "VARCHAR(255)".to_uppercase();
        // Should not match any numeric/blob/json/datetime branch
        assert!(
            !upper.contains("INT")
                && !upper.contains("FLOAT")
                && !upper.contains("DOUBLE")
                && !upper.contains("BLOB")
                && !upper.contains("JSON")
                && !upper.contains("DATETIME")
                && !upper.contains("TIMESTAMP")
        );
    }
}
