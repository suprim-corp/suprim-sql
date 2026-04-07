use std::collections::HashMap;
use std::time::Instant;

use sqlx::postgres::{PgArguments, PgPool};
use sqlx::{query::Query, AssertSqlSafe, Postgres};

use crate::db::types::{DbValue, QueryResult};
use crate::error::{AppError, Result};

use super::type_mapping::rows_to_query_result;

/// Execute a raw SQL string and return results.
pub async fn execute(pool: &PgPool, sql: &str) -> Result<QueryResult> {
    let start = Instant::now();
    let sql_owned = sql.to_string();
    let rows = sqlx::query(AssertSqlSafe(sql_owned))
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::query(sql, e.to_string()))?;
    Ok(rows_to_query_result(rows, start.elapsed()))
}

/// Execute SQL with positional parameters.
pub async fn execute_with_params(
    pool: &PgPool,
    sql: &str,
    params: Vec<DbValue>,
) -> Result<QueryResult> {
    let start = Instant::now();
    let sql_owned = sql.to_string();
    let mut query = sqlx::query(AssertSqlSafe(sql_owned));
    for param in params {
        query = match param {
            DbValue::Null => query.bind(Option::<String>::None),
            DbValue::Bool(b) => query.bind(b),
            DbValue::Int(i) => query.bind(i),
            DbValue::Float(f) => query.bind(f),
            DbValue::Text(s) => query.bind(s),
            DbValue::Bytes(b) => query.bind(b),
            DbValue::Json(v) => query.bind(v),
            DbValue::Timestamp(t) => query.bind(t),
        };
    }
    let rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::query(sql, e.to_string()))?;
    Ok(rows_to_query_result(rows, start.elapsed()))
}

/// Fetch a page of rows from a table inside a READ ONLY transaction
/// to prevent mutations via injected WHERE/ORDER BY clauses.
pub async fn table_data(
    pool: &PgPool,
    schema: Option<&str>,
    table: &str,
    page: u32,
    page_size: u32,
    where_clause: Option<&str>,
    order_clause: Option<&str>,
) -> Result<QueryResult> {
    let start = Instant::now();
    let offset = page * page_size;
    let schema_prefix = schema
        .map(|s| format!("\"{}\".", s))
        .unwrap_or_default();

    let mut sql = format!(
        "SELECT * FROM {}\"{}\"",
        schema_prefix, table
    );

    if let Some(w) = where_clause {
        let w = w.trim();
        if !w.is_empty() {
            sql.push_str(&format!("\nWHERE {}", w));
        }
    }

    if let Some(o) = order_clause {
        let o = o.trim();
        if !o.is_empty() {
            sql.push_str(&format!("\nORDER BY {}", o));
        }
    }

    sql.push_str(&format!("\nLIMIT {} OFFSET {}", page_size, offset));

    // Run inside a READ ONLY transaction to block any mutation via SQL injection.
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::query(&sql, e.to_string()))?;

    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::query(&sql, e.to_string()))?;

    let rows = sqlx::query(AssertSqlSafe(sql.clone()))
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::query(&sql, e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::query(&sql, e.to_string()))?;

    Ok(rows_to_query_result(rows, start.elapsed()))
}

/// Insert a new row. Returns rows affected.
pub async fn insert_row(
    pool: &PgPool,
    table: &str,
    values: HashMap<String, DbValue>,
) -> Result<u64> {
    let cols: Vec<&str> = values.keys().map(|s| s.as_str()).collect();
    let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("${i}")).collect();

    let sql = format!(
        "INSERT INTO \"{}\" ({}) VALUES ({})",
        table,
        cols.iter()
            .map(|c| format!("\"{}\"", c))
            .collect::<Vec<_>>()
            .join(", "),
        placeholders.join(", ")
    );

    let mut query = sqlx::query(AssertSqlSafe(sql.clone()));
    for col in &cols {
        let val = values.get(*col).unwrap();
        query = bind_db_value(query, val);
    }

    let result = query
        .execute(pool)
        .await
        .map_err(|e| AppError::query(&sql, e.to_string()))?;

    Ok(result.rows_affected())
}

/// Update an existing row identified by primary key values.
pub async fn update_row(
    pool: &PgPool,
    table: &str,
    pk: HashMap<String, DbValue>,
    changes: HashMap<String, DbValue>,
) -> Result<u64> {
    let mut idx = 1usize;

    let set_clause: Vec<String> = changes
        .keys()
        .map(|k| {
            let s = format!("\"{}\" = ${idx}", k);
            idx += 1;
            s
        })
        .collect();

    let where_clause: Vec<String> = pk
        .keys()
        .map(|k| {
            let s = format!("\"{}\" = ${idx}", k);
            idx += 1;
            s
        })
        .collect();

    let sql = format!(
        "UPDATE \"{}\" SET {} WHERE {}",
        table,
        set_clause.join(", "),
        where_clause.join(" AND ")
    );

    let mut query = sqlx::query(AssertSqlSafe(sql.clone()));
    for val in changes.values() {
        query = bind_db_value(query, val);
    }
    for val in pk.values() {
        query = bind_db_value(query, val);
    }

    let result = query
        .execute(pool)
        .await
        .map_err(|e| AppError::query(&sql, e.to_string()))?;

    Ok(result.rows_affected())
}

/// Delete a row identified by primary key values.
pub async fn delete_row(
    pool: &PgPool,
    table: &str,
    pk: HashMap<String, DbValue>,
) -> Result<u64> {
    let where_clause: Vec<String> = pk
        .keys()
        .enumerate()
        .map(|(i, k)| format!("\"{}\" = ${}", k, i + 1))
        .collect();

    let sql = format!(
        "DELETE FROM \"{}\" WHERE {}",
        table,
        where_clause.join(" AND ")
    );

    let mut query = sqlx::query(AssertSqlSafe(sql.clone()));
    for val in pk.values() {
        query = bind_db_value(query, val);
    }

    let result = query
        .execute(pool)
        .await
        .map_err(|e| AppError::query(&sql, e.to_string()))?;

    Ok(result.rows_affected())
}

// ─── Internal binding helper ──────────────────────────────────────────────────

fn bind_db_value<'q>(
    query: Query<'q, Postgres, PgArguments>,
    val: &'q DbValue,
) -> Query<'q, Postgres, PgArguments> {
    match val {
        DbValue::Null => query.bind(Option::<String>::None),
        DbValue::Bool(b) => query.bind(*b),
        DbValue::Int(i) => query.bind(*i),
        DbValue::Float(f) => query.bind(*f),
        DbValue::Text(s) => query.bind(s.as_str()),
        DbValue::Bytes(b) => query.bind(b.as_slice()),
        DbValue::Json(v) => query.bind(v.clone()),
        DbValue::Timestamp(t) => query.bind(*t),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #[test]
    fn insert_sql_structure() {
        let cols = vec!["id", "name"];
        let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("${i}")).collect();
        let sql = format!(
            "INSERT INTO \"users\" ({}) VALUES ({})",
            cols.iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", "),
            placeholders.join(", ")
        );
        assert!(sql.contains("INSERT INTO \"users\""));
        assert!(sql.contains("$1"));
        assert!(sql.contains("$2"));
    }

    #[test]
    fn delete_sql_structure() {
        let pk_cols = vec!["id"];
        let where_clause: Vec<String> = pk_cols
            .iter()
            .enumerate()
            .map(|(i, k)| format!("\"{}\" = ${}", k, i + 1))
            .collect();
        let sql = format!(
            "DELETE FROM \"users\" WHERE {}",
            where_clause.join(" AND ")
        );
        assert_eq!(sql, "DELETE FROM \"users\" WHERE \"id\" = $1");
    }

    #[test]
    fn table_data_sql_no_schema() {
        let page = 0u32;
        let page_size = 50u32;
        let offset = page * page_size;
        let schema_prefix = "";
        let sql = format!(
            "SELECT * FROM {}\"{}\"\nLIMIT {} OFFSET {}",
            schema_prefix, "users", page_size, offset
        );
        assert_eq!(sql, "SELECT * FROM \"users\"\nLIMIT 50 OFFSET 0");
    }

    #[test]
    fn table_data_sql_with_schema() {
        let page = 1u32;
        let page_size = 25u32;
        let offset = page * page_size;
        let schema_prefix = "\"public\".";
        let sql = format!(
            "SELECT * FROM {}\"{}\"\nLIMIT {} OFFSET {}",
            schema_prefix, "orders", page_size, offset
        );
        assert_eq!(sql, "SELECT * FROM \"public\".\"orders\"\nLIMIT 25 OFFSET 25");
    }

    #[test]
    fn update_sql_structure() {
        let set = vec!["name = $1"];
        let whr = vec!["id = $2"];
        let sql = format!(
            "UPDATE \"users\" SET {} WHERE {}",
            set.join(", "),
            whr.join(" AND ")
        );
        assert_eq!(sql, "UPDATE \"users\" SET name = $1 WHERE id = $2");
    }
}
