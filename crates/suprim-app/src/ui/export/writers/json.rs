//! JSON writer — export a `QueryResult` to JSON.

use std::io::Write;
use std::path::Path;

use suprim_core::db::values::{DbValue, QueryResult};

use super::super::json_options::JsonOptions;

/// Export a single `QueryResult` to JSON.
pub fn export(result: &QueryResult, path: &Path, opts: &JsonOptions) -> std::io::Result<()> {
    let col_names: Vec<&str> = result.columns.iter().map(|c| c.name.as_str()).collect();

    let rows: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (name, val) in col_names.iter().zip(row.iter()) {
                let is_null = matches!(val, DbValue::Null);
                if is_null && !opts.include_null_values {
                    continue;
                }
                obj.insert(name.to_string(), json_value(val, opts.all_as_strings));
            }
            serde_json::Value::Object(obj)
        })
        .collect();

    let json = if opts.pretty_print {
        serde_json::to_string_pretty(&rows)
    } else {
        serde_json::to_string(&rows)
    }
    .map_err(std::io::Error::other)?;

    let mut f = super::create_writer(path, opts.gzip)?;
    f.write_all(json.as_bytes())
}

fn json_value(val: &DbValue, all_as_strings: bool) -> serde_json::Value {
    if all_as_strings {
        return match val {
            DbValue::Null => serde_json::Value::Null,
            other => serde_json::Value::String(other.display()),
        };
    }
    match val {
        DbValue::Null => serde_json::Value::Null,
        DbValue::Bool(b) => serde_json::Value::Bool(*b),
        DbValue::Int(i) => serde_json::json!(i),
        DbValue::Float(f) => serde_json::json!(f),
        DbValue::Text(s) => serde_json::Value::String(s.clone()),
        DbValue::Json(v) => v.clone(),
        DbValue::Bytes(b) => {
            let hex: String = b.iter().map(|byte| format!("{:02x}", byte)).collect();
            serde_json::Value::String(format!("\\x{hex}"))
        }
        DbValue::Timestamp(t) => {
            serde_json::Value::String(t.format("%Y-%m-%d %H:%M:%S").to_string())
        }
    }
}
