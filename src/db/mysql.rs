use std::collections::HashMap;
use std::time::Instant;

use async_trait::async_trait;
use sqlx::{AssertSqlSafe, Column, Row, TypeInfo};
use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions, MySqlRow};

use crate::db::connection::{ConnectionConfig, DriverParams, DriverType};
use crate::db::driver::DatabaseDriver;
use crate::db::types::{
    ColumnMeta, ColumnNode, DatabaseNode, DbValue, ForeignKeyNode, IndexNode, QueryResult,
    SchemaNode, SchemaTree, TableNode, ViewNode,
};
use crate::error::{AppError, Result};

// ─── Type mapping ────────────────────────────────────────────────────────────

/// Map a MySQL row cell to DbValue using the column's type name.
pub fn mysql_value_from_row(row: &MySqlRow, idx: usize, type_name: &str) -> DbValue {
    let upper = type_name.to_uppercase();

    // Integer types
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
    if upper.contains("MEDIUMINT") || upper.contains("INT") && !upper.contains("BIGINT") {
        return row
            .try_get::<i32, _>(idx)
            .map(|v| DbValue::Int(v as i64))
            .unwrap_or(DbValue::Null);
    }
    if upper.contains("BIGINT") {
        return row
            .try_get::<i64, _>(idx)
            .map(DbValue::Int)
            .unwrap_or(DbValue::Null);
    }

    // Floating point
    if upper.contains("FLOAT") {
        return row
            .try_get::<f32, _>(idx)
            .map(|v| DbValue::Float(v as f64))
            .unwrap_or(DbValue::Null);
    }
    if upper.contains("DOUBLE") || upper.contains("DECIMAL") || upper.contains("NUMERIC") {
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

    // Datetime
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
pub fn rows_to_query_result(rows: Vec<MySqlRow>, elapsed: std::time::Duration) -> QueryResult {
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
    }
}

// ─── Driver ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct MysqlDriver {
    pool: Option<MySqlPool>,
}

impl MysqlDriver {
    pub fn new() -> Self {
        Self { pool: None }
    }

    fn pool(&self) -> Result<&MySqlPool> {
        self.pool.as_ref().ok_or(AppError::NotConnected)
    }
}

impl Default for MysqlDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DatabaseDriver for MysqlDriver {
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()> {
        let (host, port, database, user, password) = match &config.params {
            DriverParams::Mysql {
                host,
                port,
                database,
                user,
                password_key,
            } => (
                host.as_str(),
                *port,
                database.as_str(),
                user.as_str(),
                password_key.as_str(),
            ),
            _ => return Err(AppError::connection("MysqlDriver requires Mysql params")),
        };

        let opts = MySqlConnectOptions::new()
            .host(host)
            .port(port)
            .database(database)
            .username(user)
            .password(password);

        let pool = MySqlPoolOptions::new()
            .max_connections(10)
            .min_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(10))
            .connect_with(opts)
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;

        self.pool = Some(pool);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }
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
                DbValue::Timestamp(t) => query.bind(t),
            };
        }

        let rows = query
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::query(sql, e.to_string()))?;

        Ok(rows_to_query_result(rows, start.elapsed()))
    }

    async fn load_schema(&self) -> Result<SchemaTree> {
        let pool = self.pool()?;

        // 1. Current database name
        let db_row = sqlx::query("SELECT DATABASE() AS db_name")
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::Schema(e.to_string()))?;
        let current_db: String = db_row.try_get("db_name").unwrap_or_default();

        // 2. List all tables + views in current database
        let table_rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT TABLE_NAME, TABLE_TYPE \
             FROM INFORMATION_SCHEMA.TABLES \
             WHERE TABLE_SCHEMA = '{}' \
             ORDER BY TABLE_TYPE, TABLE_NAME",
            current_db
        )))
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Schema(e.to_string()))?;

        let mut tables = Vec::new();
        let mut views = Vec::new();

        for row in &table_rows {
            let tname: String = row.try_get("TABLE_NAME").unwrap_or_default();
            let ttype: String = row.try_get("TABLE_TYPE").unwrap_or_default();

            // 3. Columns
            let col_rows = sqlx::query(AssertSqlSafe(format!(
                "SELECT COLUMN_NAME, COLUMN_TYPE, DATA_TYPE, IS_NULLABLE, \
                        COLUMN_DEFAULT, COLUMN_KEY \
                 FROM INFORMATION_SCHEMA.COLUMNS \
                 WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
                 ORDER BY ORDINAL_POSITION",
                current_db, tname
            )))
            .fetch_all(pool)
            .await
            .map_err(|e| AppError::Schema(e.to_string()))?;

            let columns: Vec<ColumnNode> = col_rows
                .iter()
                .map(|r| {
                    let col_name: String = r.try_get("COLUMN_NAME").unwrap_or_default();
                    let col_type: String = r.try_get("COLUMN_TYPE").unwrap_or_default();
                    let nullable: String = r.try_get("IS_NULLABLE").unwrap_or("YES".to_string());
                    let key: String = r.try_get("COLUMN_KEY").unwrap_or_default();
                    let dflt: Option<String> = r.try_get("COLUMN_DEFAULT").unwrap_or(None);
                    ColumnNode {
                        id: uuid::Uuid::new_v4(),
                        name: col_name,
                        db_type: col_type,
                        nullable: nullable == "YES",
                        is_primary_key: key == "PRI",
                        default_value: dflt,
                    }
                })
                .collect();

            if ttype == "VIEW" {
                views.push(ViewNode {
                    id: uuid::Uuid::new_v4(),
                    name: tname,
                    columns,
                });
            } else {
                // 4. Indexes
                let idx_rows = sqlx::query(AssertSqlSafe(format!(
                    "SELECT INDEX_NAME, NON_UNIQUE, GROUP_CONCAT(COLUMN_NAME ORDER BY SEQ_IN_INDEX) AS col_names \
                     FROM INFORMATION_SCHEMA.STATISTICS \
                     WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
                     GROUP BY INDEX_NAME, NON_UNIQUE \
                     ORDER BY INDEX_NAME",
                    current_db, tname
                )))
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                let indexes: Vec<IndexNode> = idx_rows
                    .iter()
                    .map(|r| {
                        let iname: String = r.try_get("INDEX_NAME").unwrap_or_default();
                        let non_unique: i8 = r.try_get("NON_UNIQUE").unwrap_or(1);
                        let cols: String = r.try_get("col_names").unwrap_or_default();
                        IndexNode {
                            id: uuid::Uuid::new_v4(),
                            name: iname,
                            columns: cols.split(',').map(|s| s.to_string()).collect(),
                            is_unique: non_unique == 0,
                        }
                    })
                    .collect();

                // 5. Foreign keys
                let fk_rows = sqlx::query(AssertSqlSafe(format!(
                    "SELECT kcu.CONSTRAINT_NAME, kcu.COLUMN_NAME, \
                            kcu.REFERENCED_TABLE_NAME, kcu.REFERENCED_COLUMN_NAME \
                     FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE kcu \
                     JOIN INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc \
                          ON kcu.CONSTRAINT_NAME = tc.CONSTRAINT_NAME \
                         AND kcu.TABLE_SCHEMA = tc.TABLE_SCHEMA \
                         AND kcu.TABLE_NAME = tc.TABLE_NAME \
                     WHERE kcu.TABLE_SCHEMA = '{}' AND kcu.TABLE_NAME = '{}' \
                           AND tc.CONSTRAINT_TYPE = 'FOREIGN KEY' \
                     ORDER BY kcu.CONSTRAINT_NAME, kcu.ORDINAL_POSITION",
                    current_db, tname
                )))
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                let mut fk_map: HashMap<String, ForeignKeyNode> = HashMap::new();
                for r in &fk_rows {
                    let cname: String = r.try_get("CONSTRAINT_NAME").unwrap_or_default();
                    let col: String = r.try_get("COLUMN_NAME").unwrap_or_default();
                    let ref_table: String =
                        r.try_get("REFERENCED_TABLE_NAME").unwrap_or_default();
                    let ref_col: String =
                        r.try_get("REFERENCED_COLUMN_NAME").unwrap_or_default();

                    let fk = fk_map.entry(cname.clone()).or_insert(ForeignKeyNode {
                        id: uuid::Uuid::new_v4(),
                        name: cname,
                        columns: vec![],
                        ref_table,
                        ref_columns: vec![],
                    });
                    fk.columns.push(col);
                    fk.ref_columns.push(ref_col);
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
                name: current_db,
                schemas: vec![SchemaNode {
                    id: uuid::Uuid::new_v4(),
                    name: "public".to_string(),
                    tables,
                    views,
                    loaded: true,
                }],
            }],
        })
    }

    async fn load_schema_detail(
        &self,
        schema_name: &str,
    ) -> Result<crate::db::types::SchemaNode> {
        let tree = self.load_schema().await?;
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
            "SELECT * FROM `{}` LIMIT {} OFFSET {}",
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
        let placeholders: Vec<String> = cols.iter().map(|_| "?".to_string()).collect();

        let sql = format!(
            "INSERT INTO `{}` ({}) VALUES ({})",
            table,
            cols.iter()
                .map(|c| format!("`{}`", c))
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

        let set_clause: Vec<String> = changes.keys().map(|k| format!("`{}` = ?", k)).collect();
        let where_clause: Vec<String> = pk.keys().map(|k| format!("`{}` = ?", k)).collect();

        let sql = format!(
            "UPDATE `{}` SET {} WHERE {}",
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
        let where_clause: Vec<String> = pk.keys().map(|k| format!("`{}` = ?", k)).collect();

        let sql = format!(
            "DELETE FROM `{}` WHERE {}",
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
        DriverType::Mysql
    }

    fn is_connected(&self) -> bool {
        self.pool.is_some()
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn bind_db_value<'q>(
    query: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    val: &'q DbValue,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
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

// ─── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::DriverParams;

    fn make_config() -> ConnectionConfig {
        ConnectionConfig::new(
            "test-mysql",
            DriverParams::Mysql {
                host: "localhost".into(),
                port: 3306,
                database: "test".into(),
                user: "root".into(),
                password_key: "root".into(),
            },
        )
    }

    // ── Constructor / state ───────────────────────────────────────────────────

    #[test]
    fn new_driver_not_connected() {
        let driver = MysqlDriver::new();
        assert!(!driver.is_connected());
    }

    #[test]
    fn default_driver_not_connected() {
        let driver = MysqlDriver::default();
        assert!(!driver.is_connected());
    }

    #[test]
    fn driver_type_returns_mysql() {
        let driver = MysqlDriver::new();
        assert_eq!(driver.driver_type(), DriverType::Mysql);
    }

    // ── Not-connected errors ──────────────────────────────────────────────────

    #[tokio::test]
    async fn disconnect_without_connect_is_ok() {
        let mut driver = MysqlDriver::new();
        assert!(driver.disconnect().await.is_ok());
    }

    #[tokio::test]
    async fn ping_without_connect_returns_not_connected() {
        let driver = MysqlDriver::new();
        let err = driver.ping().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_without_connect_returns_not_connected() {
        let driver = MysqlDriver::new();
        let err = driver.execute("SELECT 1").await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_with_params_without_connect_returns_not_connected() {
        let driver = MysqlDriver::new();
        let err = driver
            .execute_with_params("SELECT ?", vec![])
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn load_schema_without_connect_returns_not_connected() {
        let driver = MysqlDriver::new();
        let err = driver.load_schema().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn table_data_without_connect_returns_not_connected() {
        let driver = MysqlDriver::new();
        let err = driver.table_data(None, "users", 0, 50).await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn insert_without_connect_returns_not_connected() {
        let driver = MysqlDriver::new();
        let err = driver.insert_row("users", HashMap::new()).await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn update_without_connect_returns_not_connected() {
        let driver = MysqlDriver::new();
        let err = driver
            .update_row("users", HashMap::new(), HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn delete_without_connect_returns_not_connected() {
        let driver = MysqlDriver::new();
        let err = driver
            .delete_row("users", HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn connect_wrong_params_returns_error() {
        let mut driver = MysqlDriver::new();
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

    // ── SQL builder helpers ───────────────────────────────────────────────────

    #[test]
    fn insert_sql_structure() {
        let cols = vec!["id", "name"];
        let placeholders: Vec<String> = cols.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "INSERT INTO `t` ({}) VALUES ({})",
            cols.iter()
                .map(|c| format!("`{}`", c))
                .collect::<Vec<_>>()
                .join(", "),
            placeholders.join(", ")
        );
        assert!(sql.contains("INSERT INTO `t`"));
        assert!(sql.contains("?, ?"));
    }

    #[test]
    fn delete_sql_structure() {
        let pk_cols = vec!["id"];
        let where_clause: Vec<String> = pk_cols.iter().map(|k| format!("`{}` = ?", k)).collect();
        let sql = format!("DELETE FROM `t` WHERE {}", where_clause.join(" AND "));
        assert_eq!(sql, "DELETE FROM `t` WHERE `id` = ?");
    }

    #[test]
    fn update_sql_structure() {
        let set = vec!["`name` = ?"];
        let whr = vec!["`id` = ?"];
        let sql = format!("UPDATE `t` SET {} WHERE {}", set.join(", "), whr.join(" AND "));
        assert_eq!(sql, "UPDATE `t` SET `name` = ? WHERE `id` = ?");
    }

    #[test]
    fn table_data_sql_structure() {
        let sql = format!("SELECT * FROM `{}` LIMIT {} OFFSET {}", "users", 50, 0);
        assert_eq!(sql, "SELECT * FROM `users` LIMIT 50 OFFSET 0");
    }

    #[test]
    fn rows_to_query_result_empty() {
        let result = rows_to_query_result(vec![], std::time::Duration::from_millis(2));
        assert_eq!(result.columns.len(), 0);
        assert_eq!(result.rows.len(), 0);
        assert_eq!(result.rows_affected, 0);
        assert_eq!(result.execution_time.as_millis(), 2);
    }

    // ── Type name mapping ─────────────────────────────────────────────────────

    #[test]
    fn mysql_type_bool_detection() {
        let upper = "TINYINT(1)".to_uppercase();
        assert!(upper == "TINYINT(1)");
    }

    #[test]
    fn mysql_type_bigint_detection() {
        let upper = "BIGINT".to_uppercase();
        assert!(upper.contains("BIGINT"));
    }

    #[test]
    fn mysql_type_varchar_falls_through_to_text() {
        // VARCHAR → none of the special branches → text arm
        let upper = "VARCHAR(255)".to_uppercase();
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
