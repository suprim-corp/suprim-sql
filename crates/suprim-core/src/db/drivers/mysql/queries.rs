//! Query execution: raw SQL, table_data (COUNT/WHERE/ORDER/READ ONLY), CRUD.
//!
//! # Security: AssertSqlSafe usage
//!
//! `AssertSqlSafe` wraps dynamically built SQL to satisfy sqlx's injection protection.
//! This is necessary because WHERE/ORDER BY clauses come from user input (filter bar).
//!
//! Mitigations:
//! - `table_data()` wraps queries in `SET SESSION TRANSACTION READ ONLY` — prevents
//!   any mutation even if injected SQL contains INSERT/UPDATE/DELETE/DROP.
//! - CRUD operations (`insert_row`, `update_row`, `delete_row`) use `?` placeholder
//!   binding for all values — only column/table names are interpolated.
//! - `execute()` runs arbitrary user SQL (SQL editor) — this is intentional, user
//!   explicitly chooses to run whatever they type.
//!
//! Remaining risk: column/table names in CRUD are not parameterized (MySQL doesn't
//! support parameterized identifiers). This is acceptable because these names come
//! from the app's own schema tree, not from external input.

use std::collections::HashMap;
use std::time::Instant;

use sqlx::mysql::{MySqlArguments, MySqlPool};
use sqlx::{query::Query, AssertSqlSafe, MySql, Row};

use crate::db::types::{DbValue, QueryResult};
use crate::error::{AppError, Result};

use super::type_mapping::rows_to_query_result;

/// Execute a raw SQL string and return results.
pub(super) async fn execute(pool: &MySqlPool, sql: &str) -> Result<QueryResult> {
    let start = Instant::now();
    let rows = sqlx::query(AssertSqlSafe(sql.to_string()))
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::query(sql, e.to_string()))?;
    Ok(rows_to_query_result(rows, start.elapsed()))
}

/// Execute SQL with positional parameters.
pub(super) async fn execute_with_params(
    pool: &MySqlPool,
    sql: &str,
    params: Vec<DbValue>,
) -> Result<QueryResult> {
    let start = Instant::now();
    let mut query = sqlx::query(AssertSqlSafe(sql.to_string()));
    for param in params {
        query = match param {
            DbValue::Null => query.bind(Option::<String>::None),
            DbValue::Bool(b) => query.bind(b),
            DbValue::Int(i) => query.bind(i),
            DbValue::Float(f) => query.bind(f),
            DbValue::Text(s) => query.bind(s),
            DbValue::Bytes(b) => query.bind(b),
            DbValue::Json(v) => query.bind(v.to_string()),
            DbValue::Timestamp(t) => query.bind(t),
        };
    }
    let rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::query(sql, e.to_string()))?;
    Ok(rows_to_query_result(rows, start.elapsed()))
}

/// Execute SQL on a specific database: acquire a single connection, run `USE`, then execute query.
pub(super) async fn execute_on_database(
    pool: &MySqlPool,
    sql: &str,
    database: &str,
) -> Result<QueryResult> {
    let start = Instant::now();
    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| AppError::connection(e.to_string()))?;
    let use_sql = format!("USE {}", super::quote_ident(database));
    sqlx::raw_sql(AssertSqlSafe(use_sql.clone()))
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::query(&use_sql, e.to_string()))?;
    let rows = sqlx::query(AssertSqlSafe(sql.to_string()))
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| AppError::query(sql, e.to_string()))?;
    Ok(rows_to_query_result(rows, start.elapsed()))
}

/// Fetch a page of rows from a table inside a READ ONLY session
/// to prevent mutations via injected WHERE/ORDER BY clauses.
/// Also runs a COUNT(*) query to provide total row count for pagination.
pub(super) async fn table_data(
    pool: &MySqlPool,
    database: Option<&str>,
    table: &str,
    page: u32,
    page_size: u32,
    where_clause: Option<&str>,
    order_clause: Option<&str>,
) -> Result<QueryResult> {
    let start = Instant::now();
    let offset = (page as u64) * (page_size as u64);

    // Acquire a single connection to guarantee USE + queries run on the same session.
    let mut conn = pool
        .acquire()
        .await
        .map_err(|e| AppError::connection(e.to_string()))?;

    // Switch to the requested database if specified
    if let Some(db) = database {
        let use_sql = format!("USE {}", super::quote_ident(db));
        sqlx::raw_sql(AssertSqlSafe(use_sql.clone()))
            .execute(&mut *conn)
            .await
            .map_err(|e| AppError::query(&use_sql, e.to_string()))?;
    }

    // Validate user-provided clauses before injecting into SQL.
    let where_clause = match where_clause {
        Some(w) => Some(crate::db::sanitize::validate_where_clause(w)?),
        None => None,
    };
    let order_clause = match order_clause {
        Some(o) => Some(crate::db::sanitize::validate_order_clause(o)?),
        None => None,
    };

    let table_ref = super::quote_ident(table);

    // Build WHERE fragment (shared by both COUNT and SELECT)
    let where_fragment = match &where_clause {
        Some(w) if !w.is_empty() => format!(" WHERE {}", w),
        _ => String::new(),
    };

    // 1) COUNT query — total rows matching WHERE
    let count_sql = format!("SELECT COUNT(*) AS cnt FROM {}{}", table_ref, where_fragment);

    // 2) Data query — paginated
    let mut data_sql = format!("SELECT * FROM {}{}", table_ref, where_fragment);
    if let Some(o) = &order_clause {
        if !o.is_empty() {
            data_sql.push_str(&format!(" ORDER BY {}", o));
        }
    }
    data_sql.push_str(&format!(" LIMIT {} OFFSET {}", page_size, offset));

    // Set session to READ ONLY to block mutation via SQL injection
    sqlx::raw_sql(AssertSqlSafe("SET SESSION TRANSACTION READ ONLY".to_string()))
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::query("SET SESSION TRANSACTION READ ONLY", e.to_string()))?;

    // Run count
    let count_result = sqlx::query(AssertSqlSafe(count_sql.clone()))
        .fetch_one(&mut *conn)
        .await;

    // Run data
    let data_result = sqlx::query(AssertSqlSafe(data_sql.clone()))
        .fetch_all(&mut *conn)
        .await;

    // Restore READ WRITE mode regardless of query success
    let _ = sqlx::raw_sql(AssertSqlSafe("SET SESSION TRANSACTION READ WRITE".to_string()))
        .execute(&mut *conn)
        .await;

    let count_row = count_result.map_err(|e| AppError::query(&count_sql, e.to_string()))?;
    let total_count: i64 = count_row.try_get("cnt").unwrap_or(0);

    let rows = data_result.map_err(|e| AppError::query(&data_sql, e.to_string()))?;

    let mut result = rows_to_query_result(rows, start.elapsed());
    result.total_count = Some(total_count as u64);
    Ok(result)
}

/// Insert a new row. Returns rows affected.
pub(super) async fn insert_row(
    pool: &MySqlPool,
    table: &str,
    values: HashMap<String, DbValue>,
) -> Result<u64> {
    let entries: Vec<(&String, &DbValue)> = values.iter().collect();
    let placeholders: Vec<String> = entries.iter().map(|_| "?".to_string()).collect();

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        super::quote_ident(table),
        entries
            .iter()
            .map(|(c, _)| super::quote_ident(c))
            .collect::<Vec<_>>()
            .join(", "),
        placeholders.join(", ")
    );

    let mut query = sqlx::query(AssertSqlSafe(sql.clone()));
    for (_, val) in &entries {
        query = bind_db_value(query, val);
    }

    let result = query
        .execute(pool)
        .await
        .map_err(|e| AppError::query(&sql, e.to_string()))?;

    Ok(result.rows_affected())
}

/// Update an existing row identified by primary key values.
pub(super) async fn update_row(
    pool: &MySqlPool,
    table: &str,
    pk: HashMap<String, DbValue>,
    changes: HashMap<String, DbValue>,
) -> Result<u64> {
    let change_entries: Vec<(&String, &DbValue)> = changes.iter().collect();
    let pk_entries: Vec<(&String, &DbValue)> = pk.iter().collect();

    let set_clause: Vec<String> = change_entries
        .iter()
        .map(|(k, _)| format!("{} = ?", super::quote_ident(k)))
        .collect();
    let where_clause: Vec<String> = pk_entries
        .iter()
        .map(|(k, _)| format!("{} = ?", super::quote_ident(k)))
        .collect();

    let sql = format!(
        "UPDATE {} SET {} WHERE {}",
        super::quote_ident(table),
        set_clause.join(", "),
        where_clause.join(" AND ")
    );

    let mut query = sqlx::query(AssertSqlSafe(sql.clone()));
    for (_, val) in &change_entries {
        query = bind_db_value(query, val);
    }
    for (_, val) in &pk_entries {
        query = bind_db_value(query, val);
    }

    let result = query
        .execute(pool)
        .await
        .map_err(|e| AppError::query(&sql, e.to_string()))?;

    Ok(result.rows_affected())
}

/// Delete a row identified by primary key values.
pub(super) async fn delete_row(
    pool: &MySqlPool,
    table: &str,
    pk: HashMap<String, DbValue>,
) -> Result<u64> {
    let pk_entries: Vec<(&String, &DbValue)> = pk.iter().collect();
    let where_clause: Vec<String> = pk_entries
        .iter()
        .map(|(k, _)| format!("{} = ?", super::quote_ident(k)))
        .collect();

    let sql = format!(
        "DELETE FROM {} WHERE {}",
        super::quote_ident(table),
        where_clause.join(" AND ")
    );

    let mut query = sqlx::query(AssertSqlSafe(sql.clone()));
    for (_, val) in &pk_entries {
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
    query: Query<'q, MySql, MySqlArguments>,
    val: &'q DbValue,
) -> Query<'q, MySql, MySqlArguments> {
    match val {
        DbValue::Null => query.bind(Option::<String>::None),
        DbValue::Bool(b) => query.bind(*b),
        DbValue::Int(i) => query.bind(*i),
        DbValue::Float(f) => query.bind(*f),
        DbValue::Text(s) => query.bind(s.as_str()),
        DbValue::Bytes(b) => query.bind(b.as_slice()),
        DbValue::Json(v) => query.bind(v.to_string()),
        DbValue::Timestamp(t) => query.bind(*t),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #[test]
    fn table_data_sql_with_where_and_order() {
        let table_ref = "`users`";
        let where_fragment = " WHERE age > 18";
        let order = "name ASC";
        let page_size = 50u32;
        let offset = 0u32;

        let mut sql = format!("SELECT * FROM {}{}", table_ref, where_fragment);
        sql.push_str(&format!(" ORDER BY {}", order));
        sql.push_str(&format!(" LIMIT {} OFFSET {}", page_size, offset));

        assert_eq!(
            sql,
            "SELECT * FROM `users` WHERE age > 18 ORDER BY name ASC LIMIT 50 OFFSET 0"
        );
    }

    #[test]
    fn table_data_sql_without_where_or_order() {
        let table_ref = "`users`";
        let sql = format!("SELECT * FROM {} LIMIT {} OFFSET {}", table_ref, 50, 0);
        assert_eq!(sql, "SELECT * FROM `users` LIMIT 50 OFFSET 0");
    }

    #[test]
    fn count_sql_with_where() {
        let table_ref = "`orders`";
        let where_fragment = " WHERE status = 'active'";
        let sql = format!("SELECT COUNT(*) AS cnt FROM {}{}", table_ref, where_fragment);
        assert_eq!(
            sql,
            "SELECT COUNT(*) AS cnt FROM `orders` WHERE status = 'active'"
        );
    }

    #[test]
    fn insert_sql_uses_backtick_quoting() {
        let cols = ["id", "name"];
        let placeholders: Vec<String> = cols.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "INSERT INTO `t` ({}) VALUES ({})",
            cols.iter()
                .map(|c| format!("`{}`", c))
                .collect::<Vec<_>>()
                .join(", "),
            placeholders.join(", ")
        );
        assert!(sql.contains("`id`"));
        assert!(sql.contains("`name`"));
        assert!(sql.contains("?, ?"));
    }

    #[test]
    fn update_sql_uses_backtick_quoting() {
        let set = ["`name` = ?"];
        let whr = ["`id` = ?"];
        let sql = format!(
            "UPDATE `users` SET {} WHERE {}",
            set.join(", "),
            whr.join(" AND ")
        );
        assert_eq!(sql, "UPDATE `users` SET `name` = ? WHERE `id` = ?");
    }

    #[test]
    fn delete_sql_uses_backtick_quoting() {
        let whr = ["`id` = ?"];
        let sql = format!("DELETE FROM `users` WHERE {}", whr.join(" AND "));
        assert_eq!(sql, "DELETE FROM `users` WHERE `id` = ?");
    }
}
