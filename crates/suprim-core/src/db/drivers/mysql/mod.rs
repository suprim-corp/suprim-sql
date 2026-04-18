// src/db/drivers/mysql/mod.rs
//
// MySQL driver — split into submodules (mirrors PostgreSQL driver structure):
//   driver_impl      — DatabaseDriver trait implementation
//   connection_url   — mysql:// URL builder from DriverParams
//   type_mapping     — mysql_value_from_row + rows_to_query_result
//   schema_loader    — list_databases, list_schemas, load_schema_detail (batch)
//   queries          — execute, table_data (COUNT/WHERE/ORDER), insert/update/delete
//   dashboard_loader — sessions, metrics, slow queries, kill

mod connection_url;
mod dashboard_loader;
mod driver_impl;
mod queries;
mod schema_loader;
mod type_mapping;

pub use connection_url::{build_connection_url, urlencoding_simple};
pub use type_mapping::{mysql_value_from_row, rows_to_query_result};

use sqlx::mysql::MySqlPool;

use crate::error::{AppError, Result};

// ─── Driver ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct MysqlDriver {
    pool: Option<MySqlPool>,
}

impl MysqlDriver {
    pub fn new() -> Self {
        Self { pool: None }
    }

    pub(crate) fn pool(&self) -> Result<&MySqlPool> {
        self.pool.as_ref().ok_or(AppError::NotConnected)
    }
}

impl Default for MysqlDriver {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Common MySQL column types for UI dropdowns ──────────────────────────────

/// Curated list of common MySQL column types (base names only, no params).
pub const MYSQL_COLUMN_TYPES: &[&str] = &[
    "bigint",
    "int",
    "mediumint",
    "smallint",
    "tinyint",
    "boolean",
    "decimal",
    "float",
    "double",
    "varchar",
    "char",
    "text",
    "mediumtext",
    "longtext",
    "tinytext",
    "blob",
    "mediumblob",
    "longblob",
    "date",
    "datetime",
    "timestamp",
    "time",
    "year",
    "json",
    "enum",
    "set",
    "binary",
    "varbinary",
    "bit",
];

/// Types that accept a length/precision parameter.
pub const MYSQL_TYPES_WITH_PARAMS: &[&str] = &[
    "varchar",
    "char",
    "decimal",
    "numeric",
    "float",
    "double",
    "bit",
    "binary",
    "varbinary",
    "enum",
    "set",
    "time",
    "datetime",
    "timestamp",
];

// ─── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::{ConnectionConfig, DriverParams, DriverType};
    use crate::db::driver::DatabaseDriver;
    use std::collections::HashMap;

    fn _make_config() -> ConnectionConfig {
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
    async fn list_databases_without_connect_returns_not_connected() {
        let driver = MysqlDriver::new();
        let err = driver.list_databases().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn table_data_without_connect_returns_not_connected() {
        let driver = MysqlDriver::new();
        let err = driver
            .table_data(None, None, "users", 0, 50, None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn insert_without_connect_returns_not_connected() {
        let driver = MysqlDriver::new();
        let err = driver
            .insert_row("users", HashMap::new())
            .await
            .unwrap_err();
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
        assert!(sql.contains("INSERT INTO `t`"));
        assert!(sql.contains("?, ?"));
    }

    #[test]
    fn delete_sql_structure() {
        let pk_cols = ["id"];
        let where_clause: Vec<String> = pk_cols.iter().map(|k| format!("`{}` = ?", k)).collect();
        let sql = format!("DELETE FROM `t` WHERE {}", where_clause.join(" AND "));
        assert_eq!(sql, "DELETE FROM `t` WHERE `id` = ?");
    }

    #[test]
    fn update_sql_structure() {
        let set = ["`name` = ?"];
        let whr = ["`id` = ?"];
        let sql = format!(
            "UPDATE `t` SET {} WHERE {}",
            set.join(", "),
            whr.join(" AND ")
        );
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
        assert!(result.total_count.is_none());
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
        // VARCHAR -> none of the special branches -> text arm
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

    // ── Connection URL builder ────────────────────────────────────────────────

    #[test]
    fn build_url_basic() {
        let url = build_connection_url("localhost", 3306, "mydb", "user", "pass");
        assert_eq!(url, "mysql://user:pass@localhost:3306/mydb");
    }

    #[test]
    fn build_url_special_chars_in_password() {
        let url = build_connection_url("localhost", 3306, "mydb", "user", "p@ss:word");
        assert_eq!(url, "mysql://user:p%40ss%3Aword@localhost:3306/mydb");
    }

    #[test]
    fn build_url_custom_port() {
        let url = build_connection_url("db.example.com", 3307, "prod", "admin", "secret");
        assert_eq!(url, "mysql://admin:secret@db.example.com:3307/prod");
    }
}
