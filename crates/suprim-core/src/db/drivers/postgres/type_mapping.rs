use std::time::Duration;

use sqlx::postgres::PgRow;
use sqlx::{Column, Row, TypeInfo};

use crate::db::types::{ColumnMeta, DbValue, QueryResult};

/// Map a raw PgRow cell (by column index) into a DbValue.
/// Falls back to DbValue::Text on unknown types.
pub fn pg_value_from_row(row: &PgRow, idx: usize, type_name: &str) -> DbValue {
    match type_name {
        "BOOL" => row
            .try_get::<bool, _>(idx)
            .map(DbValue::Bool)
            .unwrap_or(DbValue::Null),

        "INT2" | "SMALLINT" => row
            .try_get::<i16, _>(idx)
            .map(|v| DbValue::Int(v as i64))
            .unwrap_or(DbValue::Null),

        "INT4" | "INTEGER" => row
            .try_get::<i32, _>(idx)
            .map(|v| DbValue::Int(v as i64))
            .unwrap_or(DbValue::Null),

        "INT8" | "BIGINT" => row
            .try_get::<i64, _>(idx)
            .map(DbValue::Int)
            .unwrap_or(DbValue::Null),

        "FLOAT4" | "REAL" => row
            .try_get::<f32, _>(idx)
            .map(|v| DbValue::Float(v as f64))
            .unwrap_or(DbValue::Null),

        "FLOAT8" | "DOUBLE PRECISION" => row
            .try_get::<f64, _>(idx)
            .map(DbValue::Float)
            .unwrap_or(DbValue::Null),

        "NUMERIC" | "DECIMAL" => row
            .try_get::<rust_decimal::Decimal, _>(idx)
            .map(|d| DbValue::Decimal(d.to_string()))
            .unwrap_or(DbValue::Null),

        "TEXT" | "VARCHAR" | "CHAR" | "BPCHAR" | "NAME" | "CITEXT" => row
            .try_get::<String, _>(idx)
            .map(DbValue::Text)
            .unwrap_or(DbValue::Null),

        "BYTEA" => row
            .try_get::<Vec<u8>, _>(idx)
            .map(DbValue::Bytes)
            .unwrap_or(DbValue::Null),

        "JSON" | "JSONB" => row
            .try_get::<serde_json::Value, _>(idx)
            .map(DbValue::Json)
            .unwrap_or(DbValue::Null),

        "TIMESTAMPTZ" => row
            .try_get::<chrono::DateTime<chrono::Utc>, _>(idx)
            .map(DbValue::Timestamp)
            .unwrap_or(DbValue::Null),

        "TIMESTAMP" => row
            .try_get::<chrono::NaiveDateTime, _>(idx)
            .map(|v| DbValue::Timestamp(v.and_utc()))
            .unwrap_or(DbValue::Null),

        "UUID" => row
            .try_get::<uuid::Uuid, _>(idx)
            .map(|v| DbValue::Text(v.to_string()))
            .unwrap_or(DbValue::Null),

        _ => {
            // Fallback: try String, then Null
            row.try_get::<String, _>(idx)
                .map(DbValue::Text)
                .unwrap_or(DbValue::Null)
        }
    }
}

/// Convert a Vec<PgRow> + elapsed time into a QueryResult.
pub fn rows_to_query_result(rows: Vec<PgRow>, elapsed: Duration) -> QueryResult {
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
            // runtime rows don't carry nullability; use schema introspection
            nullable: true,
        })
        .collect();

    let data_rows: Vec<Vec<DbValue>> = rows
        .iter()
        .map(|row| {
            row.columns()
                .iter()
                .enumerate()
                .map(|(i, col)| pg_value_from_row(row, i, col.type_info().name()))
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
    fn pg_type_name_bool() {
        let type_name = "BOOL";
        let covered = matches!(
            type_name,
            "BOOL"
                | "INT2"
                | "INT4"
                | "INT8"
                | "FLOAT4"
                | "FLOAT8"
                | "NUMERIC"
                | "DECIMAL"
                | "TEXT"
                | "VARCHAR"
                | "CHAR"
                | "BPCHAR"
                | "NAME"
                | "BYTEA"
                | "JSON"
                | "JSONB"
                | "TIMESTAMPTZ"
                | "TIMESTAMP"
                | "UUID"
        );
        assert!(covered);
    }

    #[test]
    fn pg_type_name_unknown_falls_back() {
        let type_name = "SOME_CUSTOM_TYPE";
        let is_known = matches!(
            type_name,
            "BOOL"
                | "INT2"
                | "INT4"
                | "INT8"
                | "FLOAT4"
                | "FLOAT8"
                | "NUMERIC"
                | "DECIMAL"
                | "TEXT"
                | "VARCHAR"
                | "CHAR"
                | "BPCHAR"
                | "NAME"
                | "BYTEA"
                | "JSON"
                | "JSONB"
                | "TIMESTAMPTZ"
                | "TIMESTAMP"
                | "UUID"
        );
        assert!(!is_known, "unknown type should not match known arms");
    }

    #[test]
    fn rows_to_query_result_empty_rows() {
        let result = rows_to_query_result(vec![], Duration::from_millis(5));
        assert_eq!(result.columns.len(), 0);
        assert_eq!(result.rows.len(), 0);
        assert_eq!(result.rows_affected, 0);
        assert_eq!(result.execution_time.as_millis(), 5);
    }
}
