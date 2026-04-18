use std::collections::HashMap;
use std::time::Instant;

use sqlx::postgres::{PgArguments, PgPool};
use sqlx::{query::Query, AssertSqlSafe, Postgres, Row};

use crate::db::types::{DbValue, QueryResult};
use crate::error::{AppError, Result};

use super::type_mapping::rows_to_query_result;

/// Quote a SQL identifier: escape internal `"` → `""`, wrap in double quotes.
fn quote_ident(s: &str) -> String {
    let clean = s.trim_matches('"').replace('"', "\"\"");
    format!("\"{}\"", clean)
}

/// Quote a table reference for SQL: handles `schema.table` → `"schema"."table"`,
/// and plain `table` → `"table"`. Strips existing quotes, escapes internal `"`.
fn quote_table(table: &str) -> String {
    table
        .split('.')
        .map(|part| quote_ident(part))
        .collect::<Vec<_>>()
        .join(".")
}

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
            DbValue::Decimal(s) => {
                if let Ok(d) = s.parse::<rust_decimal::Decimal>() {
                    query.bind(d)
                } else {
                    query.bind(s)
                }
            }
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
/// Also runs a COUNT(*) query to provide total row count for pagination.
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
    let offset = (page as u64) * (page_size as u64);

    let table_ref = match schema {
        Some(s) => format!("{}.{}", quote_ident(s), quote_ident(table)),
        None => quote_ident(table),
    };

    // Validate user-provided clauses before injecting into SQL.
    let where_clause = match where_clause {
        Some(w) => Some(crate::db::sanitize::validate_where_clause(w)?),
        None => None,
    };
    let order_clause = match order_clause {
        Some(o) => Some(crate::db::sanitize::validate_order_clause(o)?),
        None => None,
    };

    // Build WHERE fragment (shared by both COUNT and SELECT)
    let where_fragment = match &where_clause {
        Some(w) if !w.is_empty() => format!("\nWHERE {}", w),
        _ => String::new(),
    };

    // 1) COUNT query — total rows matching WHERE
    let count_sql = format!("SELECT COUNT(*) AS cnt FROM {}{}", table_ref, where_fragment);

    // 2) Data query — paginated
    let mut data_sql = format!("SELECT * FROM {}{}", table_ref, where_fragment);
    if let Some(o) = &order_clause {
        if !o.is_empty() {
            data_sql.push_str(&format!("\nORDER BY {}", o));
        }
    }
    data_sql.push_str(&format!("\nLIMIT {} OFFSET {}", page_size, offset));

    // Run inside a READ ONLY transaction to block any mutation via SQL injection.
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::query(&data_sql, e.to_string()))?;

    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::query(&data_sql, e.to_string()))?;

    // Run count
    let count_row: (i64,) = sqlx::query_as(AssertSqlSafe(count_sql.clone()))
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| AppError::query(&count_sql, e.to_string()))?;
    let total_count = count_row.0 as u64;

    // Run data
    let rows = sqlx::query(AssertSqlSafe(data_sql.clone()))
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| AppError::query(&data_sql, e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| AppError::query(&data_sql, e.to_string()))?;

    let mut result = rows_to_query_result(rows, start.elapsed());
    result.total_count = Some(total_count);

    // Query information_schema for accurate column nullability metadata.
    let schema_name = schema.unwrap_or("public");
    let nullable_sql = "SELECT column_name, is_nullable FROM information_schema.columns WHERE table_schema = $1 AND table_name = $2 ORDER BY ordinal_position";
    let nullable_rows = sqlx::query(nullable_sql)
        .bind(schema_name)
        .bind(table)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    let nullable_map: std::collections::HashMap<String, bool> = nullable_rows
        .iter()
        .filter_map(|r| {
            let name: String = r.try_get("column_name").ok()?;
            let nullable: String = r.try_get("is_nullable").ok()?;
            Some((name, nullable == "YES"))
        })
        .collect();

    for col in &mut result.columns {
        if let Some(&n) = nullable_map.get(&col.name) {
            col.nullable = n;
        }
    }

    Ok(result)
}

/// Insert a new row. Returns rows affected.
pub async fn insert_row(
    pool: &PgPool,
    table: &str,
    values: HashMap<String, DbValue>,
) -> Result<u64> {
    // Collect into Vec to guarantee consistent key-value pairing.
    let pairs: Vec<(&String, &DbValue)> = values.iter().collect();
    let placeholders: Vec<String> = (1..=pairs.len()).map(|i| format!("${i}")).collect();

    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quote_table(table),
        pairs
            .iter()
            .map(|(k, _)| quote_ident(k))
            .collect::<Vec<_>>()
            .join(", "),
        placeholders.join(", ")
    );

    let mut query = sqlx::query(AssertSqlSafe(sql.clone()));
    for (_, val) in &pairs {
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
    // Collect into Vecs to guarantee consistent key-value pairing.
    let change_pairs: Vec<(&String, &DbValue)> = changes.iter().collect();
    let pk_pairs: Vec<(&String, &DbValue)> = pk.iter().collect();

    let mut idx = 1usize;

    let set_clause: Vec<String> = change_pairs
        .iter()
        .map(|(k, _)| {
            let s = format!("{} = ${idx}", quote_ident(k));
            idx += 1;
            s
        })
        .collect();

    let where_clause: Vec<String> = pk_pairs
        .iter()
        .map(|(k, _)| {
            let s = format!("{} = ${idx}", quote_ident(k));
            idx += 1;
            s
        })
        .collect();

    let sql = format!(
        "UPDATE {} SET {} WHERE {}",
        quote_table(table),
        set_clause.join(", "),
        where_clause.join(" AND ")
    );

    let mut query = sqlx::query(AssertSqlSafe(sql.clone()));
    for (_, val) in &change_pairs {
        query = bind_db_value(query, val);
    }
    for (_, val) in &pk_pairs {
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
    // Collect into Vec to guarantee consistent key-value pairing.
    let pk_pairs: Vec<(&String, &DbValue)> = pk.iter().collect();

    let where_clause: Vec<String> = pk_pairs
        .iter()
        .enumerate()
        .map(|(i, (k, _))| format!("{} = ${}", quote_ident(k), i + 1))
        .collect();

    let sql = format!(
        "DELETE FROM {} WHERE {}",
        quote_table(table),
        where_clause.join(" AND ")
    );

    let mut query = sqlx::query(AssertSqlSafe(sql.clone()));
    for (_, val) in &pk_pairs {
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
        DbValue::Decimal(s) => {
            if let Ok(d) = s.parse::<rust_decimal::Decimal>() {
                query.bind(d)
            } else {
                query.bind(s.as_str())
            }
        }
        DbValue::Text(s) => query.bind(s.as_str()),
        DbValue::Bytes(b) => query.bind(b.as_slice()),
        DbValue::Json(v) => query.bind(v.clone()),
        DbValue::Timestamp(t) => query.bind(*t),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{quote_ident, quote_table};

    #[test]
    fn quote_ident_plain() {
        assert_eq!(quote_ident("name"), "\"name\"");
    }

    #[test]
    fn quote_ident_internal_double_quote() {
        assert_eq!(quote_ident("my\"col"), "\"my\"\"col\"");
    }

    #[test]
    fn quote_ident_already_quoted() {
        assert_eq!(quote_ident("\"name\""), "\"name\"");
    }

    #[test]
    fn insert_sql_structure() {
        let cols = vec!["id", "name"];
        let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("${i}")).collect();
        let sql = format!(
            "INSERT INTO \"users\" ({}) VALUES ({})",
            cols.iter()
                .map(|c| quote_ident(c))
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
            .map(|(i, k)| format!("{} = ${}", quote_ident(k), i + 1))
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
        let table_ref = quote_ident("users");
        let sql = format!(
            "SELECT * FROM {}\nLIMIT {} OFFSET {}",
            table_ref, page_size, offset
        );
        assert_eq!(sql, "SELECT * FROM \"users\"\nLIMIT 50 OFFSET 0");
    }

    #[test]
    fn table_data_sql_with_schema() {
        let page = 1u32;
        let page_size = 25u32;
        let offset = page * page_size;
        let table_ref = format!("{}.{}", quote_ident("public"), quote_ident("orders"));
        let sql = format!(
            "SELECT * FROM {}\nLIMIT {} OFFSET {}",
            table_ref, page_size, offset
        );
        assert_eq!(sql, "SELECT * FROM \"public\".\"orders\"\nLIMIT 25 OFFSET 25");
    }

    #[test]
    fn update_sql_structure() {
        let set = vec![format!("{} = $1", quote_ident("name"))];
        let whr = vec![format!("{} = $2", quote_ident("id"))];
        let sql = format!(
            "UPDATE \"users\" SET {} WHERE {}",
            set.join(", "),
            whr.join(" AND ")
        );
        assert_eq!(sql, "UPDATE \"users\" SET \"name\" = $1 WHERE \"id\" = $2");
    }

    #[test]
    fn quote_table_plain() {
        assert_eq!(quote_table("users"), "\"users\"");
    }

    #[test]
    fn quote_table_schema_dot_table() {
        assert_eq!(quote_table("public.accounts"), "\"public\".\"accounts\"");
    }

    #[test]
    fn quote_table_already_quoted() {
        assert_eq!(
            quote_table("\"public\".\"accounts\""),
            "\"public\".\"accounts\""
        );
    }

    #[test]
    fn quote_table_internal_double_quote() {
        assert_eq!(
            quote_table("my\"table"),
            "\"my\"\"table\""
        );
    }
}
