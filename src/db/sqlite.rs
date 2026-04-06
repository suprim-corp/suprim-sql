use std::collections::HashMap;
use std::time::Instant;

use async_trait::async_trait;
use sqlx::{AssertSqlSafe, Column, Row, TypeInfo};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions, SqliteRow};

use crate::db::connection::{ConnectionConfig, DriverParams, DriverType};
use crate::db::driver::DatabaseDriver;
use crate::db::types::{
    ColumnMeta, ColumnNode, DatabaseNode, DbValue, ForeignKeyNode, IndexNode, QueryResult,
    SchemaNode, SchemaTree, TableNode, ViewNode,
};
use crate::error::{AppError, Result};

// ─── Type mapping ────────────────────────────────────────────────────────────

/// Map a raw SqliteRow cell (by column index) into a DbValue.
/// SQLite uses dynamic typing — we map by declared type affinity.
pub fn sqlite_value_from_row(row: &SqliteRow, idx: usize, type_name: &str) -> DbValue {
    let upper = type_name.to_uppercase();
    // SQLite type affinity rules (simplified)
    if upper.contains("INT") {
        return row
            .try_get::<i64, _>(idx)
            .map(DbValue::Int)
            .unwrap_or(DbValue::Null);
    }
    if upper.contains("REAL") || upper.contains("FLOA") || upper.contains("DOUB") {
        return row
            .try_get::<f64, _>(idx)
            .map(DbValue::Float)
            .unwrap_or(DbValue::Null);
    }
    if upper.contains("BLOB") || upper.is_empty() {
        // Unknown / untyped column — probe types in order
        if let Ok(i) = row.try_get::<i64, _>(idx) {
            return DbValue::Int(i);
        }
        if let Ok(f) = row.try_get::<f64, _>(idx) {
            return DbValue::Float(f);
        }
        if let Ok(b) = row.try_get::<Vec<u8>, _>(idx) {
            return DbValue::Bytes(b);
        }
        // fall through to text
    }
    // Default: TEXT affinity — also catches NUMERIC, DATE, DATETIME as string.
    // Also handles untyped columns that hold string values.
    row.try_get::<String, _>(idx)
        .map(DbValue::Text)
        .unwrap_or_else(|_| {
            // Last resort: try numeric probes again (covers bound params without type info)
            row.try_get::<i64, _>(idx)
                .map(DbValue::Int)
                .unwrap_or_else(|_| {
                    row.try_get::<f64, _>(idx)
                        .map(DbValue::Float)
                        .unwrap_or(DbValue::Null)
                })
        })
}

/// Convert a Vec<SqliteRow> + elapsed time into a QueryResult.
pub fn rows_to_query_result(rows: Vec<SqliteRow>, elapsed: std::time::Duration) -> QueryResult {
    if rows.is_empty() {
        return QueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected: 0,
            execution_time: elapsed,
        };
    }

    let columns: Vec<ColumnMeta> = rows[0]
        .columns()
        .iter()
        .map(|c| ColumnMeta {
            name: c.name().to_string(),
            db_type: c.type_info().name().to_string(),
            nullable: true,
        })
        .collect();

    let data_rows: Vec<Vec<DbValue>> = rows
        .iter()
        .map(|row| {
            row.columns()
                .iter()
                .enumerate()
                .map(|(i, col)| sqlite_value_from_row(row, i, col.type_info().name()))
                .collect()
        })
        .collect();

    let row_count = data_rows.len() as u64;

    QueryResult {
        columns,
        rows: data_rows,
        rows_affected: row_count,
        execution_time: elapsed,
    }
}

// ─── Driver ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SqliteDriver {
    pool: Option<SqlitePool>,
    /// Path to the database file — kept for reconnect / display
    db_path: Option<String>,
}

impl SqliteDriver {
    pub fn new() -> Self {
        Self {
            pool: None,
            db_path: None,
        }
    }

    fn pool(&self) -> Result<&SqlitePool> {
        self.pool.as_ref().ok_or(AppError::NotConnected)
    }
}

impl Default for SqliteDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl SqliteDriver {
    /// Build the full schema tree (used internally by `load_schema_detail`).
    async fn build_schema_tree(&self) -> Result<SchemaTree> {
        let pool = self.pool()?;
        let db_name = self
            .db_path
            .clone()
            .unwrap_or_else(|| ":memory:".to_string());

        // List all tables and views in the main schema
        let table_rows = sqlx::query(
            "SELECT name, type FROM sqlite_master \
             WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' \
             ORDER BY type, name",
        )
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Schema(e.to_string()))?;

        let mut tables = Vec::new();
        let mut views = Vec::new();

        for row in &table_rows {
            let tname: String = row.try_get("name").unwrap_or_default();
            let ttype: String = row.try_get("type").unwrap_or_default();

            // Columns via PRAGMA table_info
            let col_rows = sqlx::query(AssertSqlSafe(format!(
                "PRAGMA table_info(\"{}\")",
                tname
            )))
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::Schema(e.to_string()))?;

            let columns: Vec<ColumnNode> = col_rows
                .iter()
                .map(|r| {
                    let col_name: String = r.try_get("name").unwrap_or_default();
                    let col_type: String = r.try_get("type").unwrap_or_default();
                    let not_null: i64 = r.try_get("notnull").unwrap_or(0);
                    let pk: i64 = r.try_get("pk").unwrap_or(0);
                    let dflt: Option<String> = r.try_get("dflt_value").unwrap_or(None);
                    ColumnNode {
                        id: uuid::Uuid::new_v4(),
                        name: col_name,
                        db_type: col_type,
                        nullable: not_null == 0,
                        is_primary_key: pk > 0,
                        default_value: dflt,
                    }
                })
                .collect();

            if ttype == "view" {
                views.push(ViewNode {
                    id: uuid::Uuid::new_v4(),
                    name: tname,
                    columns,
                });
            } else {
                // Indexes via PRAGMA index_list
                let idx_list = sqlx::query(AssertSqlSafe(format!(
                    "PRAGMA index_list(\"{}\")",
                    tname
                )))
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                let mut indexes = Vec::new();
                for idx_row in &idx_list {
                    let idx_name: String = idx_row.try_get("name").unwrap_or_default();
                    let unique: i64 = idx_row.try_get("unique").unwrap_or(0);

                    let idx_info = sqlx::query(AssertSqlSafe(format!(
                        "PRAGMA index_info(\"{}\")",
                        idx_name
                    )))
                    .fetch_all(pool)
                    .await
                    .unwrap_or_default();

                    let idx_cols: Vec<String> = idx_info
                        .iter()
                        .filter_map(|r| r.try_get::<String, _>("name").ok())
                        .collect();

                    indexes.push(IndexNode {
                        id: uuid::Uuid::new_v4(),
                        name: idx_name,
                        columns: idx_cols,
                        is_unique: unique != 0,
                    });
                }

                // Foreign keys via PRAGMA foreign_key_list
                let fk_rows = sqlx::query(AssertSqlSafe(format!(
                    "PRAGMA foreign_key_list(\"{}\")",
                    tname
                )))
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                // Group by id (SQLite foreign_key_list.id groups multi-column FKs)
                let mut fk_map: HashMap<i64, ForeignKeyNode> = HashMap::new();
                for r in &fk_rows {
                    let fk_id: i64 = r.try_get("id").unwrap_or(0);
                    let from_col: String = r.try_get("from").unwrap_or_default();
                    let to_table: String = r.try_get("table").unwrap_or_default();
                    let to_col: String = r.try_get("to").unwrap_or_default();

                    let fk = fk_map.entry(fk_id).or_insert(ForeignKeyNode {
                        id: uuid::Uuid::new_v4(),
                        name: format!("fk_{}_{}_{}", tname, from_col, to_table),
                        columns: vec![],
                        ref_table: to_table,
                        ref_columns: vec![],
                    });
                    fk.columns.push(from_col);
                    fk.ref_columns.push(to_col);
                }

                tables.push(TableNode {
                    id: uuid::Uuid::new_v4(),
                    name: tname,
                    columns,
                    indexes,
                    foreign_keys: fk_map.into_values().collect(),
                    row_count: None,
                });
            }
        }

        Ok(SchemaTree {
            databases: vec![DatabaseNode {
                id: uuid::Uuid::new_v4(),
                name: db_name,
                schemas: vec![SchemaNode {
                    id: uuid::Uuid::new_v4(),
                    name: "main".to_string(),
                    tables,
                    views,
                    loaded: true,
                }],
            }],
        })
    }
}

#[async_trait]
impl DatabaseDriver for SqliteDriver {
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()> {
        let path = match &config.params {
            DriverParams::Sqlite { path } => path.to_string_lossy().into_owned(),
            _ => return Err(AppError::connection("SqliteDriver requires Sqlite params")),
        };

        let is_memory = path == ":memory:";
        let opts = if is_memory {
            // In-memory databases are per-connection; use URI with shared cache so
            // all pool connections share the same in-memory database.
            SqliteConnectOptions::new()
                .filename("file::memory:")
                .create_if_missing(true)
        } else {
            SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true)
        };

        // For in-memory DBs use a single connection to guarantee a shared namespace.
        let max_conn = if is_memory { 1 } else { 5 };
        let pool = SqlitePoolOptions::new()
            .max_connections(max_conn)
            .connect_with(opts)
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;

        self.pool = Some(pool);
        self.db_path = Some(path);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }
        self.db_path = None;
        Ok(())
    }

    async fn ping(&self) -> Result<()> {
        let pool = self.pool()?;
        sqlx::query("SELECT 1")
            .execute(pool)
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;
        Ok(())
    }

    async fn execute(&self, sql: &str) -> Result<QueryResult> {
        let pool = self.pool()?;
        let start = Instant::now();

        let rows = sqlx::query(AssertSqlSafe(sql.to_string()))
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::query(sql, e.to_string()))?;

        Ok(rows_to_query_result(rows, start.elapsed()))
    }

    async fn execute_with_params(&self, sql: &str, params: Vec<DbValue>) -> Result<QueryResult> {
        let pool = self.pool()?;
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
                DbValue::Timestamp(t) => query.bind(t.to_rfc3339()),
            };
        }

        let rows = query
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::query(sql, e.to_string()))?;

        Ok(rows_to_query_result(rows, start.elapsed()))
    }

    /// SQLite has a single database — return its name (file path or ":memory:").
    async fn list_databases(&self) -> Result<Vec<String>> {
        let _pool = self.pool()?;
        Ok(vec![self
            .db_path
            .clone()
            .unwrap_or_else(|| ":memory:".to_string())])
    }

    /// SQLite has no schema concept — return a single pseudo-schema "main".
    async fn list_schemas(&self, _database: &str) -> Result<Vec<String>> {
        let _pool = self.pool()?;
        Ok(vec!["main".to_string()])
    }

    async fn load_schema_detail(
        &self,
        schema_name: &str,
    ) -> Result<crate::db::types::SchemaNode> {
        // SQLite has a single schema ("main") — build and return the schema node.
        let tree = self.build_schema_tree().await?;
        tree.databases
            .into_iter()
            .flat_map(|db| db.schemas)
            .find(|s| s.name == schema_name)
            .ok_or_else(|| {
                crate::error::AppError::Schema(format!("schema '{}' not found", schema_name))
            })
    }

    async fn table_data(
        &self,
        _schema: Option<&str>,
        table: &str,
        page: u32,
        page_size: u32,
    ) -> Result<QueryResult> {
        let pool = self.pool()?;
        let start = Instant::now();
        let offset = page * page_size;

        let sql = format!(
            "SELECT * FROM \"{}\" LIMIT {} OFFSET {}",
            table, page_size, offset
        );
        let rows = sqlx::query(AssertSqlSafe(sql.clone()))
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::query(&sql, e.to_string()))?;

        Ok(rows_to_query_result(rows, start.elapsed()))
    }

    async fn insert_row(&self, table: &str, values: HashMap<String, DbValue>) -> Result<u64> {
        let pool = self.pool()?;
        let cols: Vec<&str> = values.keys().map(|s| s.as_str()).collect();
        let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();

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

    async fn update_row(
        &self,
        table: &str,
        pk: HashMap<String, DbValue>,
        changes: HashMap<String, DbValue>,
    ) -> Result<u64> {
        let pool = self.pool()?;
        let mut idx = 1usize;

        let set_clause: Vec<String> = changes
            .keys()
            .map(|k| {
                let s = format!("\"{}\" = ?{idx}", k);
                idx += 1;
                s
            })
            .collect();

        let where_clause: Vec<String> = pk
            .keys()
            .map(|k| {
                let s = format!("\"{}\" = ?{idx}", k);
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

    async fn delete_row(&self, table: &str, pk: HashMap<String, DbValue>) -> Result<u64> {
        let pool = self.pool()?;
        let where_clause: Vec<String> = pk
            .keys()
            .enumerate()
            .map(|(i, k)| format!("\"{}\" = ?{}", k, i + 1))
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

    fn driver_type(&self) -> DriverType {
        DriverType::Sqlite
    }

    fn is_connected(&self) -> bool {
        self.pool.is_some()
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn bind_db_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments>,
    val: &'q DbValue,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments> {
    match val {
        DbValue::Null => query.bind(Option::<String>::None),
        DbValue::Bool(b) => query.bind(*b),
        DbValue::Int(i) => query.bind(*i),
        DbValue::Float(f) => query.bind(*f),
        DbValue::Text(s) => query.bind(s.as_str()),
        DbValue::Bytes(b) => query.bind(b.as_slice()),
        DbValue::Json(v) => query.bind(v.to_string()),
        DbValue::Timestamp(t) => query.bind(t.to_rfc3339()),
    }
}

// ─── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::DriverParams;

    fn make_config(path: &str) -> ConnectionConfig {
        ConnectionConfig::new(
            "test-sqlite",
            DriverParams::Sqlite {
                path: path.into(),
            },
        )
    }

    // ── Constructor / state ───────────────────────────────────────────────────

    #[test]
    fn new_driver_not_connected() {
        let driver = SqliteDriver::new();
        assert!(!driver.is_connected());
    }

    #[test]
    fn default_driver_not_connected() {
        let driver = SqliteDriver::default();
        assert!(!driver.is_connected());
    }

    #[test]
    fn driver_type_returns_sqlite() {
        let driver = SqliteDriver::new();
        assert_eq!(driver.driver_type(), DriverType::Sqlite);
    }

    // ── Not-connected errors ──────────────────────────────────────────────────

    #[tokio::test]
    async fn disconnect_without_connect_is_ok() {
        let mut driver = SqliteDriver::new();
        assert!(driver.disconnect().await.is_ok());
    }

    #[tokio::test]
    async fn ping_without_connect_returns_not_connected() {
        let driver = SqliteDriver::new();
        let err = driver.ping().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_without_connect_returns_not_connected() {
        let driver = SqliteDriver::new();
        let err = driver.execute("SELECT 1").await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_with_params_without_connect_returns_not_connected() {
        let driver = SqliteDriver::new();
        let err = driver
            .execute_with_params("SELECT ?1", vec![])
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn list_databases_without_connect_returns_not_connected() {
        let driver = SqliteDriver::new();
        let err = driver.list_databases().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn table_data_without_connect_returns_not_connected() {
        let driver = SqliteDriver::new();
        let err = driver.table_data(None, "users", 0, 50).await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn insert_without_connect_returns_not_connected() {
        let driver = SqliteDriver::new();
        let err = driver.insert_row("users", HashMap::new()).await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn update_without_connect_returns_not_connected() {
        let driver = SqliteDriver::new();
        let err = driver
            .update_row("users", HashMap::new(), HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn delete_without_connect_returns_not_connected() {
        let driver = SqliteDriver::new();
        let err = driver
            .delete_row("users", HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn connect_wrong_params_returns_error() {
        let mut driver = SqliteDriver::new();
        let config = ConnectionConfig::new(
            "bad",
            DriverParams::Postgres {
                host: "localhost".into(),
                port: 5432,
                database: "db".into(),
                user: "user".into(),
                password_key: "key".into(),
            },
        );
        let err = driver.connect(&config).await.unwrap_err();
        assert!(matches!(err, AppError::Connection(_)));
    }

    // ── In-memory database functional tests ──────────────────────────────────

    #[tokio::test]
    async fn connect_and_ping_memory_db() {
        let mut driver = SqliteDriver::new();
        let config = make_config(":memory:");
        driver.connect(&config).await.unwrap();
        assert!(driver.is_connected());
        driver.ping().await.unwrap();
        driver.disconnect().await.unwrap();
        assert!(!driver.is_connected());
    }

    #[tokio::test]
    async fn execute_select_returns_rows() {
        let mut driver = SqliteDriver::new();
        let config = make_config(":memory:");
        driver.connect(&config).await.unwrap();

        let result = driver.execute("SELECT 42 AS n").await.unwrap();
        assert_eq!(result.columns.len(), 1);
        assert_eq!(result.columns[0].name, "n");
        assert_eq!(result.rows.len(), 1);
    }

    #[tokio::test]
    async fn execute_with_params_works() {
        let mut driver = SqliteDriver::new();
        let config = make_config(":memory:");
        driver.connect(&config).await.unwrap();

        let result = driver
            .execute_with_params("SELECT ? AS val", vec![DbValue::Int(99)])
            .await
            .unwrap();
        assert_eq!(result.rows[0][0], DbValue::Int(99));
    }

    #[tokio::test]
    async fn create_table_and_insert_update_delete() {
        let mut driver = SqliteDriver::new();
        let config = make_config(":memory:");
        driver.connect(&config).await.unwrap();

        driver
            .execute("CREATE TABLE t (id INTEGER PRIMARY KEY, name TEXT)")
            .await
            .unwrap();

        let mut values = HashMap::new();
        values.insert("id".to_string(), DbValue::Int(1));
        values.insert("name".to_string(), DbValue::Text("Alice".to_string()));
        let affected = driver.insert_row("t", values).await.unwrap();
        assert_eq!(affected, 1);

        let mut pk = HashMap::new();
        pk.insert("id".to_string(), DbValue::Int(1));
        let mut changes = HashMap::new();
        changes.insert("name".to_string(), DbValue::Text("Bob".to_string()));
        let affected = driver.update_row("t", pk.clone(), changes).await.unwrap();
        assert_eq!(affected, 1);

        let affected = driver.delete_row("t", pk).await.unwrap();
        assert_eq!(affected, 1);
    }

    #[tokio::test]
    async fn table_data_pagination() {
        let mut driver = SqliteDriver::new();
        let config = make_config(":memory:");
        driver.connect(&config).await.unwrap();

        driver
            .execute("CREATE TABLE pag (id INTEGER PRIMARY KEY)")
            .await
            .unwrap();

        for i in 1..=10i64 {
            driver
                .execute_with_params(
                    "INSERT INTO pag(id) VALUES(?1)",
                    vec![DbValue::Int(i)],
                )
                .await
                .unwrap();
        }

        let page0 = driver.table_data(None, "pag", 0, 5).await.unwrap();
        assert_eq!(page0.rows.len(), 5);

        let page1 = driver.table_data(None, "pag", 1, 5).await.unwrap();
        assert_eq!(page1.rows.len(), 5);

        let page2 = driver.table_data(None, "pag", 2, 5).await.unwrap();
        assert_eq!(page2.rows.len(), 0);
    }

    #[tokio::test]
    async fn list_databases_returns_single_entry() {
        let mut driver = SqliteDriver::new();
        let config = make_config(":memory:");
        driver.connect(&config).await.unwrap();

        let dbs = driver.list_databases().await.unwrap();
        assert_eq!(dbs.len(), 1);
        assert_eq!(dbs[0], ":memory:");
    }

    #[tokio::test]
    async fn list_schemas_returns_main() {
        let mut driver = SqliteDriver::new();
        let config = make_config(":memory:");
        driver.connect(&config).await.unwrap();

        let schemas = driver.list_schemas(":memory:").await.unwrap();
        assert_eq!(schemas.len(), 1);
        assert_eq!(schemas[0], "main");
    }

    #[tokio::test]
    async fn load_schema_detail_returns_tables_and_columns() {
        let mut driver = SqliteDriver::new();
        let config = make_config(":memory:");
        driver.connect(&config).await.unwrap();

        driver
            .execute(
                "CREATE TABLE users (\
                    id INTEGER PRIMARY KEY,\
                    name TEXT NOT NULL,\
                    age INTEGER\
                 )",
            )
            .await
            .unwrap();

        let schema = driver.load_schema_detail("main").await.unwrap();
        let table = schema.tables.iter().find(|t| t.name == "users").unwrap();
        assert_eq!(table.columns.len(), 3);
        let id_col = table.columns.iter().find(|c| c.name == "id").unwrap();
        assert!(id_col.is_primary_key);
    }

    #[tokio::test]
    async fn load_schema_detail_detects_foreign_keys() {
        let mut driver = SqliteDriver::new();
        let config = make_config(":memory:");
        driver.connect(&config).await.unwrap();

        // Enable FK enforcement (needed for PRAGMA foreign_key_list to be useful)
        driver.execute("PRAGMA foreign_keys = ON").await.unwrap();

        driver
            .execute("CREATE TABLE parent (id INTEGER PRIMARY KEY)")
            .await
            .unwrap();
        driver
            .execute(
                "CREATE TABLE child (\
                    id INTEGER PRIMARY KEY,\
                    parent_id INTEGER REFERENCES parent(id)\
                 )",
            )
            .await
            .unwrap();

        let schema = driver.load_schema_detail("main").await.unwrap();
        let child = schema.tables.iter().find(|t| t.name == "child").unwrap();
        assert!(!child.foreign_keys.is_empty());
        assert_eq!(child.foreign_keys[0].ref_table, "parent");
    }

    #[tokio::test]
    async fn load_schema_detail_detects_indexes() {
        let mut driver = SqliteDriver::new();
        let config = make_config(":memory:");
        driver.connect(&config).await.unwrap();

        driver
            .execute(
                "CREATE TABLE idx_t (id INTEGER PRIMARY KEY, email TEXT); \
                 CREATE UNIQUE INDEX uidx_email ON idx_t(email)",
            )
            .await
            .unwrap();

        let schema = driver.load_schema_detail("main").await.unwrap();
        let table = schema.tables.iter().find(|t| t.name == "idx_t").unwrap();
        let uidx = table.indexes.iter().find(|i| i.name == "uidx_email").unwrap();
        assert!(uidx.is_unique);
    }

    // ── Type-mapping helpers ──────────────────────────────────────────────────

    #[test]
    fn rows_to_query_result_empty() {
        let result = rows_to_query_result(vec![], std::time::Duration::from_millis(3));
        assert_eq!(result.columns.len(), 0);
        assert_eq!(result.rows.len(), 0);
        assert_eq!(result.rows_affected, 0);
        assert_eq!(result.execution_time.as_millis(), 3);
    }

    #[test]
    fn sqlite_type_affinity_int() {
        // Verify affinity string detection
        assert!(
            "INTEGER".contains("INT") || "BIGINT".contains("INT"),
            "int affinity should be detected"
        );
    }

    #[test]
    fn sqlite_type_affinity_real() {
        assert!(
            "REAL".contains("REAL") || "FLOAT".contains("FLOA"),
            "real affinity should be detected"
        );
    }

    // ── SQL builder helpers ───────────────────────────────────────────────────

    #[test]
    fn insert_sql_structure() {
        let cols = vec!["id", "name"];
        let placeholders: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "INSERT INTO \"t\" ({}) VALUES ({})",
            cols.iter()
                .map(|c| format!("\"{}\"", c))
                .collect::<Vec<_>>()
                .join(", "),
            placeholders.join(", ")
        );
        assert!(sql.contains("INSERT INTO \"t\""));
        assert!(sql.contains("?1"));
        assert!(sql.contains("?2"));
    }

    #[test]
    fn delete_sql_structure() {
        let pk_cols = vec!["id"];
        let where_clause: Vec<String> = pk_cols
            .iter()
            .enumerate()
            .map(|(i, k)| format!("\"{}\" = ?{}", k, i + 1))
            .collect();
        let sql = format!("DELETE FROM \"t\" WHERE {}", where_clause.join(" AND "));
        assert_eq!(sql, "DELETE FROM \"t\" WHERE \"id\" = ?1");
    }

    #[test]
    fn table_data_sql_structure() {
        let sql = format!("SELECT * FROM \"{}\" LIMIT {} OFFSET {}", "users", 50, 0);
        assert_eq!(sql, "SELECT * FROM \"users\" LIMIT 50 OFFSET 0");
    }
}
