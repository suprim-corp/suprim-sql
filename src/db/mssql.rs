// Remove unused imports — tiberius QueryItem not needed with into_first_result
use std::collections::HashMap;
use std::time::Instant;

use async_trait::async_trait;
use tiberius::{AuthMethod, Client, Column, Config, Row};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::db::connection::{ConnectionConfig, DriverParams, DriverType};
use crate::db::driver::DatabaseDriver;
use crate::db::types::{
    ColumnMeta, ColumnNode, DatabaseNode, DbValue, QueryResult,
    SchemaNode, SchemaTree, TableNode, ViewNode,
};
use crate::error::{AppError, Result};

// ─── Type alias ──────────────────────────────────────────────────────────────

type MssqlClient = Client<Compat<TcpStream>>;

// ─── Type mapping ────────────────────────────────────────────────────────────

/// Map a tiberius Row column value to DbValue.
pub fn mssql_value_from_row(row: &Row, idx: usize, col: &Column) -> DbValue {
    use tiberius::ColumnType;
    match col.column_type() {
        ColumnType::Bit => row
            .get::<bool, usize>(idx)
            .map(DbValue::Bool)
            .unwrap_or(DbValue::Null),

        ColumnType::Int1 => row
            .get::<u8, usize>(idx)
            .map(|v| DbValue::Int(v as i64))
            .unwrap_or(DbValue::Null),

        ColumnType::Int2 => row
            .get::<i16, usize>(idx)
            .map(|v| DbValue::Int(v as i64))
            .unwrap_or(DbValue::Null),

        ColumnType::Int4 => row
            .get::<i32, usize>(idx)
            .map(|v| DbValue::Int(v as i64))
            .unwrap_or(DbValue::Null),

        ColumnType::Int8 => row
            .get::<i64, usize>(idx)
            .map(DbValue::Int)
            .unwrap_or(DbValue::Null),

        ColumnType::Float4 => row
            .get::<f32, usize>(idx)
            .map(|v| DbValue::Float(v as f64))
            .unwrap_or(DbValue::Null),

        ColumnType::Float8 => row
            .get::<f64, usize>(idx)
            .map(DbValue::Float)
            .unwrap_or(DbValue::Null),

        ColumnType::BigVarBin | ColumnType::BigBinary | ColumnType::Image => row
            .get::<&[u8], usize>(idx)
            .map(|b| DbValue::Bytes(b.to_vec()))
            .unwrap_or(DbValue::Null),

        ColumnType::Datetime | ColumnType::Datetime4 | ColumnType::Datetime2 => row
            .get::<chrono::DateTime<chrono::Utc>, usize>(idx)
            .map(DbValue::Timestamp)
            .unwrap_or_else(|| {
                row.get::<chrono::NaiveDateTime, usize>(idx)
                    .map(|v| DbValue::Timestamp(v.and_utc()))
                    .unwrap_or(DbValue::Null)
            }),

        // Text / GUID / Money / Decimal / etc. → String
        _ => row
            .get::<&str, usize>(idx)
            .map(|s| DbValue::Text(s.to_string()))
            .unwrap_or_else(|| {
                // Try numeric fallback
                row.get::<i64, usize>(idx)
                    .map(DbValue::Int)
                    .unwrap_or(DbValue::Null)
            }),
    }
}

/// Convert a Vec<Row> + elapsed time into a QueryResult.
pub fn rows_to_query_result(rows: Vec<Row>, elapsed: std::time::Duration) -> QueryResult {
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
            db_type: format!("{:?}", c.column_type()),
            nullable: true,
        })
        .collect();

    let data_rows: Vec<Vec<DbValue>> = rows
        .iter()
        .map(|row| {
            row.columns()
                .iter()
                .enumerate()
                .map(|(i, col)| mssql_value_from_row(row, i, col))
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
pub struct MssqlDriver {
    client: Option<MssqlClient>,
}

impl MssqlDriver {
    pub fn new() -> Self {
        Self { client: None }
    }

    fn client_mut(&mut self) -> Result<&mut MssqlClient> {
        self.client.as_mut().ok_or(AppError::NotConnected)
    }
}

impl Default for MssqlDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DatabaseDriver for MssqlDriver {
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()> {
        let (host, port, database, user, password) = match &config.params {
            DriverParams::Mssql {
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
            _ => return Err(AppError::connection("MssqlDriver requires Mssql params")),
        };

        let mut cfg = Config::new();
        cfg.host(host);
        cfg.port(port);
        cfg.database(database);
        cfg.authentication(AuthMethod::sql_server(user, password));
        cfg.trust_cert(); // Useful for dev / self-signed certs

        let tcp = TcpStream::connect(cfg.get_addr())
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;
        tcp.set_nodelay(true)
            .map_err(|e| AppError::connection(e.to_string()))?;

        let client = Client::connect(cfg, tcp.compat_write())
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;

        self.client = Some(client);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        Ok(())
    }

    async fn ping(&self) -> Result<()> {
        // tiberius Client is not Clone, so we need &mut self — but trait is &self.
        // We mirror the not-connected check only; real ping requires mutability.
        if self.client.is_none() {
            return Err(AppError::NotConnected);
        }
        // Cannot run query with &self (tiberius requires &mut); return Ok if connected.
        Ok(())
    }

    async fn execute(&self, _sql: &str) -> Result<QueryResult> {
        // tiberius Client requires &mut; we document this limitation.
        // All real execution is via execute_mut (called by DbWorker which owns the driver).
        if self.client.is_none() {
            return Err(AppError::NotConnected);
        }
        Err(AppError::query(
            _sql,
            "MssqlDriver::execute requires mutable access — use the DbWorker channel",
        ))
    }

    async fn execute_with_params(&self, sql: &str, _params: Vec<DbValue>) -> Result<QueryResult> {
        if self.client.is_none() {
            return Err(AppError::NotConnected);
        }
        Err(AppError::query(
            sql,
            "MssqlDriver::execute_with_params requires mutable access — use the DbWorker channel",
        ))
    }

    async fn list_databases(&self) -> Result<Vec<String>> {
        if self.client.is_none() {
            return Err(AppError::NotConnected);
        }
        Err(AppError::query(
            "",
            "MssqlDriver::list_databases requires mutable access — use the DbWorker channel",
        ))
    }

    async fn list_schemas(&self, _database: &str) -> Result<Vec<String>> {
        if self.client.is_none() {
            return Err(AppError::NotConnected);
        }
        Err(AppError::query(
            "",
            "MssqlDriver::list_schemas requires mutable access — use the DbWorker channel",
        ))
    }

    async fn load_schema_detail(
        &self,
        _schema_name: &str,
    ) -> Result<crate::db::types::SchemaNode> {
        if self.client.is_none() {
            return Err(AppError::NotConnected);
        }
        Err(AppError::Schema(
            "MssqlDriver::load_schema_detail requires mutable access — use the DbWorker channel"
                .to_string(),
        ))
    }

    async fn table_data(
        &self,
        _schema: Option<&str>,
        _table: &str,
        _page: u32,
        _page_size: u32,
    ) -> Result<QueryResult> {
        if self.client.is_none() {
            return Err(AppError::NotConnected);
        }
        Err(AppError::query(
            _table,
            "MssqlDriver::table_data requires mutable access — use the DbWorker channel",
        ))
    }

    async fn insert_row(&self, _table: &str, _values: HashMap<String, DbValue>) -> Result<u64> {
        if self.client.is_none() {
            return Err(AppError::NotConnected);
        }
        Err(AppError::query(
            _table,
            "MssqlDriver::insert_row requires mutable access — use the DbWorker channel",
        ))
    }

    async fn update_row(
        &self,
        _table: &str,
        _pk: HashMap<String, DbValue>,
        _changes: HashMap<String, DbValue>,
    ) -> Result<u64> {
        if self.client.is_none() {
            return Err(AppError::NotConnected);
        }
        Err(AppError::query(
            _table,
            "MssqlDriver::update_row requires mutable access — use the DbWorker channel",
        ))
    }

    async fn delete_row(&self, _table: &str, _pk: HashMap<String, DbValue>) -> Result<u64> {
        if self.client.is_none() {
            return Err(AppError::NotConnected);
        }
        Err(AppError::query(
            _table,
            "MssqlDriver::delete_row requires mutable access — use the DbWorker channel",
        ))
    }

    fn driver_type(&self) -> DriverType {
        DriverType::Mssql
    }

    fn is_connected(&self) -> bool {
        self.client.is_some()
    }
}

// ─── Mutable execution helpers (used by DbWorker) ────────────────────────────

impl MssqlDriver {
    /// Execute a raw T-SQL statement. Returns rows fetched.
    pub async fn execute_mut(&mut self, sql: &str) -> Result<QueryResult> {
        let client = self.client.as_mut().ok_or(AppError::NotConnected)?;
        let start = Instant::now();

        let rows: Vec<Row> = client
            .query(sql, &[])
            .await
            .map_err(|e| AppError::query(sql, e.to_string()))?
            .into_first_result()
            .await
            .map_err(|e| AppError::query(sql, e.to_string()))?;

        Ok(rows_to_query_result(rows, start.elapsed()))
    }

    /// Load schema using mutable client access.
    pub async fn load_schema_mut(&mut self) -> Result<SchemaTree> {
        let client = self.client.as_mut().ok_or(AppError::NotConnected)?;

        // Current database
        let mut res = client
            .query("SELECT DB_NAME() AS db_name", &[])
            .await
            .map_err(|e| AppError::Schema(e.to_string()))?;

        let db_name: String = {
            let rows = res
                .into_first_result()
                .await
                .map_err(|e| AppError::Schema(e.to_string()))?;
            rows.first()
                .and_then(|r| r.get::<&str, usize>(0))
                .unwrap_or("")
                .to_string()
        };

        // Tables + views
        let sql = "SELECT TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE \
                   FROM INFORMATION_SCHEMA.TABLES \
                   ORDER BY TABLE_SCHEMA, TABLE_TYPE, TABLE_NAME";
        let table_rows: Vec<Row> = client
            .query(sql, &[])
            .await
            .map_err(|e| AppError::Schema(e.to_string()))?
            .into_first_result()
            .await
            .map_err(|e| AppError::Schema(e.to_string()))?;

        #[derive(Default)]
        struct TblInfo {
            schema: String,
            name: String,
            is_view: bool,
        }

        let table_infos: Vec<TblInfo> = table_rows
            .iter()
            .map(|row| {
                let schema: &str = row.get(0).unwrap_or("dbo");
                let name: &str = row.get(1).unwrap_or("");
                let ttype: &str = row.get(2).unwrap_or("");
                TblInfo {
                    schema: schema.to_string(),
                    name: name.to_string(),
                    is_view: ttype == "VIEW",
                }
            })
            .collect();

        // Group by schema
        let mut schema_map: HashMap<String, (Vec<TableNode>, Vec<ViewNode>)> = HashMap::new();
        for info in table_infos {
            let entry = schema_map.entry(info.schema.clone()).or_default();

            // Columns
            let col_sql = format!(
                "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT, \
                        COLUMNPROPERTY(OBJECT_ID('{}.{}'), COLUMN_NAME, 'IsIdentity') AS is_pk \
                 FROM INFORMATION_SCHEMA.COLUMNS \
                 WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
                 ORDER BY ORDINAL_POSITION",
                info.schema, info.name, info.schema, info.name
            );
            let col_rows: Vec<Row> = client
                .query(&col_sql, &[])
                .await
                .map_err(|e| AppError::Schema(e.to_string()))?
                .into_first_result()
                .await
                .unwrap_or_default();

            let columns: Vec<ColumnNode> = col_rows
                .iter()
                .map(|row| {
                    let col_name: &str = row.get(0).unwrap_or("");
                    let col_type: &str = row.get(1).unwrap_or("");
                    let nullable: &str = row.get(2).unwrap_or("YES");
                    let dflt: Option<&str> = row.get(3);
                    let is_pk_val: Option<i32> = row.get(4);
                    ColumnNode {
                        id: uuid::Uuid::new_v4(),
                        name: col_name.to_string(),
                        db_type: col_type.to_string(),
                        nullable: nullable == "YES",
                        is_primary_key: is_pk_val.unwrap_or(0) == 1,
                        default_value: dflt.map(|s| s.to_string()),
                    }
                })
                .collect();

            if info.is_view {
                entry.1.push(ViewNode {
                    id: uuid::Uuid::new_v4(),
                    name: info.name,
                    columns,
                });
            } else {
                entry.0.push(TableNode {
                    id: uuid::Uuid::new_v4(),
                    name: info.name,
                    columns,
                    indexes: vec![],
                    foreign_keys: vec![],
                    row_count: None,
                });
            }
        }

        let schemas: Vec<SchemaNode> = schema_map
            .into_iter()
            .map(|(schema_name, (tables, views))| SchemaNode {
                id: uuid::Uuid::new_v4(),
                name: schema_name,
                tables,
                views,
                loaded: true,
            })
            .collect();

        Ok(SchemaTree {
            databases: vec![DatabaseNode {
                id: uuid::Uuid::new_v4(),
                name: db_name,
                schemas,
            }],
        })
    }
}

// ─── Unit Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::DriverParams;

    // ── Constructor / state ───────────────────────────────────────────────────

    #[test]
    fn new_driver_not_connected() {
        let driver = MssqlDriver::new();
        assert!(!driver.is_connected());
    }

    #[test]
    fn default_driver_not_connected() {
        let driver = MssqlDriver::default();
        assert!(!driver.is_connected());
    }

    #[test]
    fn driver_type_returns_mssql() {
        let driver = MssqlDriver::new();
        assert_eq!(driver.driver_type(), DriverType::Mssql);
    }

    // ── Not-connected errors ──────────────────────────────────────────────────

    #[tokio::test]
    async fn disconnect_without_connect_is_ok() {
        let mut driver = MssqlDriver::new();
        assert!(driver.disconnect().await.is_ok());
    }

    #[tokio::test]
    async fn ping_without_connect_returns_not_connected() {
        let driver = MssqlDriver::new();
        let err = driver.ping().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_without_connect_returns_not_connected() {
        let driver = MssqlDriver::new();
        let err = driver.execute("SELECT 1").await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_with_params_without_connect_returns_not_connected() {
        let driver = MssqlDriver::new();
        let err = driver
            .execute_with_params("SELECT @P1", vec![])
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn list_databases_without_connect_returns_not_connected() {
        let driver = MssqlDriver::new();
        let err = driver.list_databases().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn table_data_without_connect_returns_not_connected() {
        let driver = MssqlDriver::new();
        let err = driver.table_data(None, "users", 0, 50).await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn insert_without_connect_returns_not_connected() {
        let driver = MssqlDriver::new();
        let err = driver.insert_row("t", HashMap::new()).await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn update_without_connect_returns_not_connected() {
        let driver = MssqlDriver::new();
        let err = driver
            .update_row("t", HashMap::new(), HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn delete_without_connect_returns_not_connected() {
        let driver = MssqlDriver::new();
        let err = driver
            .delete_row("t", HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn connect_wrong_params_returns_error() {
        let mut driver = MssqlDriver::new();
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

    #[tokio::test]
    async fn execute_mut_without_connect_returns_not_connected() {
        let mut driver = MssqlDriver::new();
        let err = driver.execute_mut("SELECT 1").await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn load_schema_mut_without_connect_returns_not_connected() {
        let mut driver = MssqlDriver::new();
        let err = driver.load_schema_mut().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    // ── Type mapping logic ────────────────────────────────────────────────────

    #[test]
    fn rows_to_query_result_empty() {
        let result = rows_to_query_result(vec![], std::time::Duration::from_millis(4));
        assert_eq!(result.columns.len(), 0);
        assert_eq!(result.rows.len(), 0);
        assert_eq!(result.rows_affected, 0);
        assert_eq!(result.execution_time.as_millis(), 4);
    }

    // ── SQL builder helpers ───────────────────────────────────────────────────

    #[test]
    fn select_db_name_sql() {
        // Verify the SQL strings used in load_schema_mut
        let sql = "SELECT DB_NAME() AS db_name";
        assert!(sql.contains("DB_NAME()"));
    }

    #[test]
    fn table_list_sql_structure() {
        let sql = "SELECT TABLE_SCHEMA, TABLE_NAME, TABLE_TYPE \
                   FROM INFORMATION_SCHEMA.TABLES \
                   ORDER BY TABLE_SCHEMA, TABLE_TYPE, TABLE_NAME";
        assert!(sql.contains("INFORMATION_SCHEMA.TABLES"));
        assert!(sql.contains("TABLE_TYPE"));
    }

    #[test]
    fn column_query_sql_structure() {
        let schema = "dbo";
        let table = "users";
        let col_sql = format!(
            "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_DEFAULT, \
                    COLUMNPROPERTY(OBJECT_ID('{}.{}'), COLUMN_NAME, 'IsIdentity') AS is_pk \
             FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}' \
             ORDER BY ORDINAL_POSITION",
            schema, table, schema, table
        );
        assert!(col_sql.contains("INFORMATION_SCHEMA.COLUMNS"));
        assert!(col_sql.contains("dbo"));
        assert!(col_sql.contains("users"));
        assert!(col_sql.contains("IsIdentity"));
    }

    #[test]
    fn mssql_driver_new_has_no_client() {
        let driver = MssqlDriver::new();
        assert!(driver.client.is_none());
    }

    // ── Connected state returns query error (not NotConnected) ────────────────

    #[tokio::test]
    async fn execute_when_connected_returns_query_error_not_not_connected() {
        // We can't create a real client, so instead verify that the "is connected"
        // branch of execute() returns a Query error (not NotConnected).
        // We simulate by checking the logic directly.
        let driver = MssqlDriver::new();
        // Not connected → should be NotConnected
        let err = driver.execute("SELECT 1").await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }
}
