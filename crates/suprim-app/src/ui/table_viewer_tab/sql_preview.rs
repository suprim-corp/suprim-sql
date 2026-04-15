use std::collections::HashMap;
/// SQL preview helpers — generate human-readable SQL for mutation operations.
/// Used by the cell editor and new-row editor to show SQL before commit.
use suprim_core::db::types::{ColumnMeta, DbValue};

/// Generate an UPDATE SQL preview string.
pub(super) fn preview_update_sql(
    schema: &str,
    table: &str,
    column_name: &str,
    new_value: &DbValue,
    pk: &HashMap<String, DbValue>,
) -> String {
    let set_clause = format!("\"{}\" = {}", column_name, format_value(new_value));
    let where_clause = build_where_clause(pk);
    format!("UPDATE \"{schema}\".\"{table}\"\nSET {set_clause}\nWHERE {where_clause};")
}

/// Generate an INSERT SQL preview string.
pub(super) fn preview_insert_sql(
    schema: &str,
    table: &str,
    values: &[(String, DbValue)],
) -> String {
    if values.is_empty() {
        return format!("INSERT INTO \"{schema}\".\"{table}\" DEFAULT VALUES;");
    }
    let cols: Vec<String> = values.iter().map(|(c, _)| format!("\"{}\"", c)).collect();
    let vals: Vec<String> = values.iter().map(|(_, v)| format_value(v)).collect();
    format!(
        "INSERT INTO \"{schema}\".\"{table}\"\n  ({})\nVALUES\n  ({});",
        cols.join(", "),
        vals.join(", ")
    )
}

/// Generate a DELETE SQL preview string.
#[allow(dead_code)]
pub(super) fn preview_delete_sql(
    schema: &str,
    table: &str,
    pk: &HashMap<String, DbValue>,
) -> String {
    let where_clause = build_where_clause(pk);
    format!("DELETE FROM \"{schema}\".\"{table}\"\nWHERE {where_clause};")
}

/// Build a WHERE clause from a primary key map.
fn build_where_clause(pk: &HashMap<String, DbValue>) -> String {
    let mut conditions: Vec<String> = pk
        .iter()
        .map(|(col, val)| {
            if val.is_null() {
                format!("\"{}\" IS NULL", col)
            } else {
                format!("\"{}\" = {}", col, format_value(val))
            }
        })
        .collect();
    conditions.sort(); // deterministic order
    conditions.join("\n  AND ")
}

/// Format a DbValue for SQL display.
fn format_value(val: &DbValue) -> String {
    match val {
        DbValue::Null => "NULL".to_string(),
        DbValue::Bool(b) => b.to_string(),
        DbValue::Int(i) => i.to_string(),
        DbValue::Float(f) => f.to_string(),
        DbValue::Text(s) => format!("'{}'", s.replace('\'', "''")),
        DbValue::Bytes(b) => format!("'\\x{}'", hex_encode(b)),
        DbValue::Json(v) => format!("'{}'::jsonb", v.to_string().replace('\'', "''")),
        DbValue::Timestamp(t) => format!("'{}'::timestamptz", t),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Build a PK map from all columns of a row (used when actual PKs are unknown).
pub(super) fn build_pk_from_row(
    columns: &[ColumnMeta],
    row_data: &[DbValue],
) -> HashMap<String, DbValue> {
    let mut pk = HashMap::new();
    for (i, col) in columns.iter().enumerate() {
        if let Some(val) = row_data.get(i) {
            pk.insert(col.name.clone(), val.clone());
        }
    }
    pk
}
