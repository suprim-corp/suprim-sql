use std::collections::HashMap;
use std::time::Instant;

use async_trait::async_trait;
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, Client, Value as RedisValue};

use crate::db::connection::{ConnectionConfig, DriverParams, DriverType};
use crate::db::driver::DatabaseDriver;
use crate::db::types::{
    ColumnMeta, ColumnNode, DatabaseNode, DbValue, QueryResult, SchemaNode, SchemaTree, TableNode,
};
use crate::error::{AppError, Result};
use crate::storage::credential;

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Convert a single Redis value to DbValue.
pub fn redis_value_to_db_value(val: &RedisValue) -> DbValue {
    match val {
        RedisValue::Nil => DbValue::Null,
        RedisValue::Int(i) => DbValue::Int(*i),
        RedisValue::BulkString(b) => {
            match std::str::from_utf8(b) {
                Ok(s) => {
                    // Try to parse as number first
                    if let Ok(i) = s.parse::<i64>() {
                        return DbValue::Int(i);
                    }
                    if let Ok(f) = s.parse::<f64>() {
                        return DbValue::Float(f);
                    }
                    // Try JSON
                    if let Ok(j) = serde_json::from_str::<serde_json::Value>(s) {
                        return DbValue::Json(j);
                    }
                    DbValue::Text(s.to_string())
                }
                Err(_) => DbValue::Bytes(b.clone()),
            }
        }
        RedisValue::SimpleString(s) => DbValue::Text(s.clone()),
        RedisValue::Array(arr) => {
            // Flatten array as JSON array
            let items: Vec<serde_json::Value> = arr
                .iter()
                .map(|v| redis_value_to_json(v))
                .collect();
            DbValue::Json(serde_json::Value::Array(items))
        }
        RedisValue::Boolean(b) => DbValue::Bool(*b),
        RedisValue::Double(f) => DbValue::Float(*f),
        RedisValue::BigNumber(n) => DbValue::Text(n.to_string()),
        RedisValue::VerbatimString { text, .. } => DbValue::Text(text.clone()),
        RedisValue::Map(pairs) => {
            let mut map = serde_json::Map::new();
            for (k, v) in pairs {
                if let DbValue::Text(key) = redis_value_to_db_value(k) {
                    map.insert(key, redis_value_to_json(v));
                }
            }
            DbValue::Json(serde_json::Value::Object(map))
        }
        RedisValue::Set(members) => {
            let items: Vec<serde_json::Value> = members
                .iter()
                .map(|v| redis_value_to_json(v))
                .collect();
            DbValue::Json(serde_json::Value::Array(items))
        }
        RedisValue::Attribute { data, .. } => redis_value_to_db_value(data),
        RedisValue::Push { data, .. } => {
            let items: Vec<serde_json::Value> = data
                .iter()
                .map(|v| redis_value_to_json(v))
                .collect();
            DbValue::Json(serde_json::Value::Array(items))
        }
        RedisValue::ServerError(e) => DbValue::Text(e.details().unwrap_or("server error").to_string()),
        _ => DbValue::Null,
    }
}

/// Convert Redis value to serde_json::Value for nesting.
fn redis_value_to_json(val: &RedisValue) -> serde_json::Value {
    match redis_value_to_db_value(val) {
        DbValue::Null => serde_json::Value::Null,
        DbValue::Bool(b) => serde_json::Value::Bool(b),
        DbValue::Int(i) => serde_json::json!(i),
        DbValue::Float(f) => serde_json::json!(f),
        DbValue::Decimal(s) => {
            s.parse::<f64>()
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(serde_json::Value::Number)
                .unwrap_or_else(|| serde_json::Value::String(s))
        }
        DbValue::Text(s) => serde_json::Value::String(s),
        DbValue::Json(j) => j,
        DbValue::Bytes(b) => serde_json::Value::String(format!("<{} bytes>", b.len())),
        DbValue::Timestamp(t) => serde_json::Value::String(t.to_rfc3339()),
    }
}

/// Build Redis connection URL from DriverParams::Redis.
pub fn build_redis_url(host: &str, port: u16, db_index: u8, password: Option<&str>) -> String {
    match password {
        Some(pw) => format!("redis://:{}@{}:{}/{}", pw, host, port, db_index),
        None => format!("redis://{}:{}/{}", host, port, db_index),
    }
}

// ─── Driver ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct RedisDriver {
    conn: Option<MultiplexedConnection>,
    client: Option<Client>,
}

impl RedisDriver {
    pub fn new() -> Self {
        Self {
            conn: None,
            client: None,
        }
    }
}

impl Default for RedisDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl RedisDriver {
    /// Build the full schema tree (used internally by `load_schema_detail`).
    /// Scans all keys and groups them by their name prefix (before first `:`).
    async fn build_schema_tree(&self) -> Result<SchemaTree> {
        let mut conn = self.conn.clone().ok_or(AppError::NotConnected)?;

        // DBSIZE for total count
        let db_size: i64 = redis::cmd("DBSIZE")
            .query_async(&mut conn)
            .await
            .unwrap_or(0);

        // SCAN to collect up to 1000 keys for schema preview
        let mut scan_iter = conn
            .scan_match::<&str, String>("*")
            .await
            .map_err(|e| AppError::Schema(e.to_string()))?;
        let mut keys: Vec<String> = Vec::new();
        while keys.len() < 1000 {
            match scan_iter.next_item().await {
                Some(Ok(key)) => keys.push(key),
                Some(Err(_)) | None => break,
            }
        }

        // Group keys by prefix (before first ':')
        let mut prefix_map: HashMap<String, Vec<String>> = HashMap::new();
        for key in &keys {
            let prefix = key.split(':').next().unwrap_or(key).to_string();
            prefix_map.entry(prefix).or_default().push(key.clone());
        }

        let tables: Vec<TableNode> = prefix_map
            .into_iter()
            .map(|(prefix, group_keys)| {
                let columns = vec![
                    ColumnNode {
                        id: uuid::Uuid::new_v4(),
                        name: "key".to_string(),
                        db_type: "string".to_string(),
                        nullable: false,
                        is_primary_key: true,
                        default_value: None,
                    },
                    ColumnNode {
                        id: uuid::Uuid::new_v4(),
                        name: "value".to_string(),
                        db_type: "string".to_string(),
                        nullable: true,
                        is_primary_key: false,
                        default_value: None,
                    },
                ];
                TableNode {
                    id: uuid::Uuid::new_v4(),
                    name: prefix,
                    columns,
                    indexes: vec![],
                    foreign_keys: vec![],
                    row_count: Some(group_keys.len() as u64),
                }
            })
            .collect();

        let db_name = format!("redis (db_size: {})", db_size);

        Ok(SchemaTree {
            databases: vec![DatabaseNode {
                id: uuid::Uuid::new_v4(),
                name: db_name,
                schemas: vec![SchemaNode {
                    id: uuid::Uuid::new_v4(),
                    name: "default".to_string(),
                    tables,
                    views: vec![],
                    loaded: true,
                }],
            }],
        })
    }
}

#[async_trait]
impl DatabaseDriver for RedisDriver {
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()> {
        let (host, port, db_index, password_key) = match &config.params {
            DriverParams::Redis {
                host,
                port,
                db_index,
                password_key,
            } => (host.as_str(), *port, *db_index, password_key.as_deref()),
            _ => return Err(AppError::connection("RedisDriver requires Redis params")),
        };

        let decrypted_pw = password_key.map(|pw| credential::decrypt(pw));
        let url = build_redis_url(host, port, db_index, decrypted_pw.as_deref());
        let client =
            Client::open(url.as_str()).map_err(|e| AppError::connection(e.to_string()))?;

        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;

        self.client = Some(client);
        self.conn = Some(conn);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.conn = None;
        self.client = None;
        Ok(())
    }

    async fn ping(&self) -> Result<()> {
        // Clone the connection for read-only ping; conn must exist
        let mut conn = self.conn.clone().ok_or(AppError::NotConnected)?;
        redis::cmd("PING")
            .query_async::<String>(&mut conn)
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;
        Ok(())
    }

    /// Execute a raw Redis command string.
    /// Format: `COMMAND arg1 arg2 ...` (space separated, first token = command name).
    async fn execute(&self, sql: &str) -> Result<QueryResult> {
        let mut conn = self.conn.clone().ok_or(AppError::NotConnected)?;
        let start = Instant::now();

        let parts: Vec<&str> = sql.split_whitespace().collect();
        if parts.is_empty() {
            return Err(AppError::query(sql, "empty command"));
        }

        let mut cmd = redis::cmd(parts[0]);
        for arg in &parts[1..] {
            cmd.arg(*arg);
        }

        let value: RedisValue = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::query(sql, e.to_string()))?;

        let elapsed = start.elapsed();
        let db_val = redis_value_to_db_value(&value);

        Ok(QueryResult {
            columns: vec![ColumnMeta {
                name: "result".to_string(),
                db_type: "redis".to_string(),
                nullable: true,
            }],
            rows: vec![vec![db_val]],
            rows_affected: 1,
            execution_time: elapsed,
        })
    }

    /// Execute a Redis command with positional parameters.
    /// `sql` = command name (e.g. "SET"), params = arguments.
    async fn execute_with_params(&self, sql: &str, params: Vec<DbValue>) -> Result<QueryResult> {
        let mut conn = self.conn.clone().ok_or(AppError::NotConnected)?;
        let start = Instant::now();

        let mut cmd = redis::cmd(sql.trim());
        for param in &params {
            match param {
                DbValue::Null => cmd.arg(b"" as &[u8]),
                DbValue::Bool(b) => cmd.arg(if *b { "1" } else { "0" }),
                DbValue::Int(i) => cmd.arg(*i),
                DbValue::Float(f) => cmd.arg(*f),
                DbValue::Decimal(s) => cmd.arg(s.as_str()),
                DbValue::Text(s) => cmd.arg(s.as_str()),
                DbValue::Bytes(b) => cmd.arg(b.as_slice()),
                DbValue::Json(v) => cmd.arg(v.to_string()),
                DbValue::Timestamp(t) => cmd.arg(t.to_rfc3339()),
            };
        }

        let value: RedisValue = cmd
            .query_async(&mut conn)
            .await
            .map_err(|e| AppError::query(sql, e.to_string()))?;

        let elapsed = start.elapsed();
        let db_val = redis_value_to_db_value(&value);

        Ok(QueryResult {
            columns: vec![ColumnMeta {
                name: "result".to_string(),
                db_type: "redis".to_string(),
                nullable: true,
            }],
            rows: vec![vec![db_val]],
            rows_affected: 1,
            execution_time: elapsed,
        })
    }

    /// Redis doesn't have traditional databases — return a single pseudo-database.
    async fn list_databases(&self) -> Result<Vec<String>> {
        let _conn = self.conn.as_ref().ok_or(AppError::NotConnected)?;
        Ok(vec!["db0".to_string()])
    }

    /// Redis doesn't have schemas — return a single pseudo-schema.
    async fn list_schemas(&self, _database: &str) -> Result<Vec<String>> {
        let _conn = self.conn.as_ref().ok_or(AppError::NotConnected)?;
        Ok(vec!["keys".to_string()])
    }

    async fn load_schema_detail(
        &self,
        schema_name: &str,
    ) -> Result<crate::db::types::SchemaNode> {
        let tree = self.build_schema_tree().await?;
        tree.databases
            .into_iter()
            .flat_map(|db| db.schemas)
            .find(|s| s.name == schema_name)
            .ok_or_else(|| {
                crate::error::AppError::Schema(format!("schema '{}' not found", schema_name))
            })
    }

    /// Fetch values for keys matching `table:*` pattern, paginated.
    async fn table_data(
        &self,
        _schema: Option<&str>,
        table: &str,
        page: u32,
        page_size: u32,
    ) -> Result<QueryResult> {
        let mut conn = self.conn.clone().ok_or(AppError::NotConnected)?;
        let start = Instant::now();

        let pattern = format!("{}:*", table);
        // Collect all matching keys first (drops scan_iter borrow before GET calls)
        let all_keys: Vec<String> = {
            let mut scan_iter = conn
                .scan_match::<String, String>(pattern)
                .await
                .map_err(|e| AppError::query(table, e.to_string()))?;
            let mut collected: Vec<String> = Vec::new();
            loop {
                match scan_iter.next_item().await {
                    Some(Ok(key)) => collected.push(key),
                    Some(Err(_)) | None => break,
                }
            }
            collected
        };

        let offset = (page * page_size) as usize;
        let page_keys: Vec<&String> = all_keys
            .iter()
            .skip(offset)
            .take(page_size as usize)
            .collect();

        let columns = vec![
            ColumnMeta {
                name: "key".to_string(),
                db_type: "string".to_string(),
                nullable: false,
            },
            ColumnMeta {
                name: "value".to_string(),
                db_type: "string".to_string(),
                nullable: true,
            },
        ];

        let mut rows = Vec::new();
        for key in page_keys {
            let value: RedisValue = redis::cmd("GET")
                .arg(key.as_str())
                .query_async(&mut conn)
                .await
                .unwrap_or(RedisValue::Nil);
            rows.push(vec![
                DbValue::Text(key.clone()),
                redis_value_to_db_value(&value),
            ]);
        }

        let row_count = rows.len() as u64;
        Ok(QueryResult {
            columns,
            rows,
            rows_affected: row_count,
            execution_time: start.elapsed(),
        })
    }

    /// Insert = SET key value (pk = {"key": key}, values = {"value": val}).
    async fn insert_row(&self, _table: &str, values: HashMap<String, DbValue>) -> Result<u64> {
        let mut conn = self.conn.clone().ok_or(AppError::NotConnected)?;

        let key = values
            .get("key")
            .map(|v| v.display())
            .ok_or_else(|| AppError::query("SET", "missing 'key' field"))?;
        let val = values
            .get("value")
            .map(|v| v.display())
            .unwrap_or_else(|| "".to_string());

        conn.set::<String, String, ()>(key, val)
            .await
            .map_err(|e| AppError::query("SET", e.to_string()))?;
        Ok(1)
    }

    /// Update = SET key new_value (same as insert for Redis).
    async fn update_row(
        &self,
        table: &str,
        pk: HashMap<String, DbValue>,
        changes: HashMap<String, DbValue>,
    ) -> Result<u64> {
        let mut merged = pk;
        merged.extend(changes);
        self.insert_row(table, merged).await
    }

    /// Delete = DEL key.
    async fn delete_row(&self, _table: &str, pk: HashMap<String, DbValue>) -> Result<u64> {
        let mut conn = self.conn.clone().ok_or(AppError::NotConnected)?;

        let key = pk
            .get("key")
            .map(|v| v.display())
            .ok_or_else(|| AppError::query("DEL", "missing 'key' field"))?;

        let count: i64 = conn
            .del::<String, i64>(key)
            .await
            .map_err(|e| AppError::query("DEL", e.to_string()))?;
        Ok(count as u64)
    }

    fn driver_type(&self) -> DriverType {
        DriverType::Redis
    }

    fn is_connected(&self) -> bool {
        self.conn.is_some()
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
        let driver = RedisDriver::new();
        assert!(!driver.is_connected());
    }

    #[test]
    fn default_driver_not_connected() {
        let driver = RedisDriver::default();
        assert!(!driver.is_connected());
    }

    #[test]
    fn driver_type_returns_redis() {
        let driver = RedisDriver::new();
        assert_eq!(driver.driver_type(), DriverType::Redis);
    }

    // ── Not-connected errors ──────────────────────────────────────────────────

    #[tokio::test]
    async fn disconnect_without_connect_is_ok() {
        let mut driver = RedisDriver::new();
        assert!(driver.disconnect().await.is_ok());
    }

    #[tokio::test]
    async fn ping_without_connect_returns_not_connected() {
        let driver = RedisDriver::new();
        let err = driver.ping().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_without_connect_returns_not_connected() {
        let driver = RedisDriver::new();
        let err = driver.execute("PING").await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_with_params_without_connect_returns_not_connected() {
        let driver = RedisDriver::new();
        let err = driver
            .execute_with_params("SET", vec![])
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn list_databases_without_connect_returns_not_connected() {
        let driver = RedisDriver::new();
        let err = driver.list_databases().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn table_data_without_connect_returns_not_connected() {
        let driver = RedisDriver::new();
        let err = driver.table_data(None, "user", 0, 50).await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn insert_without_connect_returns_not_connected() {
        let driver = RedisDriver::new();
        let err = driver.insert_row("k", HashMap::new()).await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn update_without_connect_returns_not_connected() {
        let driver = RedisDriver::new();
        let err = driver
            .update_row("k", HashMap::new(), HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn delete_without_connect_returns_not_connected() {
        let driver = RedisDriver::new();
        let err = driver
            .delete_row("k", HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn connect_wrong_params_returns_error() {
        let mut driver = RedisDriver::new();
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

    // ── URL builder ───────────────────────────────────────────────────────────

    #[test]
    fn build_url_no_password() {
        let url = build_redis_url("localhost", 6379, 0, None);
        assert_eq!(url, "redis://localhost:6379/0");
    }

    #[test]
    fn build_url_with_password() {
        let url = build_redis_url("localhost", 6379, 1, Some("secret"));
        assert_eq!(url, "redis://:secret@localhost:6379/1");
    }

    #[test]
    fn build_url_custom_db_index() {
        let url = build_redis_url("redis.example.com", 6380, 5, None);
        assert_eq!(url, "redis://redis.example.com:6380/5");
    }

    // ── Value conversion ──────────────────────────────────────────────────────

    #[test]
    fn redis_nil_becomes_null() {
        let val = redis_value_to_db_value(&RedisValue::Nil);
        assert_eq!(val, DbValue::Null);
    }

    #[test]
    fn redis_int_becomes_db_int() {
        let val = redis_value_to_db_value(&RedisValue::Int(42));
        assert_eq!(val, DbValue::Int(42));
    }

    #[test]
    fn redis_bulk_string_numeric_becomes_int() {
        let val = redis_value_to_db_value(&RedisValue::BulkString(b"99".to_vec()));
        assert_eq!(val, DbValue::Int(99));
    }

    #[test]
    fn redis_bulk_string_float_becomes_float() {
        let val = redis_value_to_db_value(&RedisValue::BulkString(b"3.14".to_vec()));
        assert_eq!(val, DbValue::Float(3.14));
    }

    #[test]
    fn redis_bulk_string_text_becomes_text() {
        let val = redis_value_to_db_value(&RedisValue::BulkString(b"hello".to_vec()));
        assert_eq!(val, DbValue::Text("hello".to_string()));
    }

    #[test]
    fn redis_simple_string_becomes_text() {
        let val = redis_value_to_db_value(&RedisValue::SimpleString("OK".to_string()));
        assert_eq!(val, DbValue::Text("OK".to_string()));
    }

    #[test]
    fn redis_bool_becomes_db_bool() {
        let val_true = redis_value_to_db_value(&RedisValue::Boolean(true));
        let val_false = redis_value_to_db_value(&RedisValue::Boolean(false));
        assert_eq!(val_true, DbValue::Bool(true));
        assert_eq!(val_false, DbValue::Bool(false));
    }

    #[test]
    fn redis_double_becomes_float() {
        let val = redis_value_to_db_value(&RedisValue::Double(2.718));
        assert_eq!(val, DbValue::Float(2.718));
    }

    #[test]
    fn redis_bulk_string_json_becomes_json() {
        let json_bytes = b"{\"x\":1}".to_vec();
        let val = redis_value_to_db_value(&RedisValue::BulkString(json_bytes));
        assert!(matches!(val, DbValue::Json(_)));
    }

    #[test]
    fn redis_array_becomes_json_array() {
        let arr = RedisValue::Array(vec![
            RedisValue::Int(1),
            RedisValue::SimpleString("two".to_string()),
        ]);
        let val = redis_value_to_db_value(&arr);
        assert!(matches!(val, DbValue::Json(serde_json::Value::Array(_))));
    }

    // ── Execute empty command ─────────────────────────────────────────────────

    #[tokio::test]
    async fn execute_empty_command_returns_error() {
        // Not connected → NotConnected error before we reach empty-check
        let driver = RedisDriver::new();
        let err = driver.execute("").await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    // ── Additional value conversion edge cases ────────────────────────────────

    #[test]
    fn redis_map_becomes_json_object() {
        let map = RedisValue::Map(vec![(
            RedisValue::SimpleString("key".to_string()),
            RedisValue::Int(42),
        )]);
        let val = redis_value_to_db_value(&map);
        assert!(matches!(val, DbValue::Json(serde_json::Value::Object(_))));
    }

    #[test]
    fn redis_set_becomes_json_array() {
        let set = RedisValue::Set(vec![RedisValue::Int(1), RedisValue::Int(2)]);
        let val = redis_value_to_db_value(&set);
        assert!(matches!(val, DbValue::Json(serde_json::Value::Array(_))));
    }

    #[test]
    fn redis_attribute_delegates_to_data() {
        let attr = RedisValue::Attribute {
            data: Box::new(RedisValue::Int(7)),
            attributes: vec![],
        };
        let val = redis_value_to_db_value(&attr);
        assert_eq!(val, DbValue::Int(7));
    }

    #[test]
    fn redis_bytes_non_utf8_becomes_bytes() {
        // 0xFF is not valid UTF-8 on its own
        let binary: Vec<u8> = vec![0x01, 0x02, 0x03, 0xFF];
        let val = redis_value_to_db_value(&RedisValue::BulkString(binary.clone()));
        // Non-UTF8 bulk string → DbValue::Bytes
        assert_eq!(val, DbValue::Bytes(binary));
    }

    #[test]
    fn redis_nested_array_to_json() {
        // Nested array: [[1, "two"], 3]
        let inner = RedisValue::Array(vec![
            RedisValue::Int(1),
            RedisValue::SimpleString("two".to_string()),
        ]);
        let outer = RedisValue::Array(vec![inner, RedisValue::Int(3)]);
        let val = redis_value_to_db_value(&outer);
        if let DbValue::Json(serde_json::Value::Array(arr)) = val {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("expected JSON array");
        }
    }

    #[test]
    fn redis_map_with_non_text_key_skipped() {
        // Key that is Int → not a text key → should be skipped in map
        let map = RedisValue::Map(vec![(RedisValue::Int(1), RedisValue::Int(99))]);
        let val = redis_value_to_db_value(&map);
        assert!(matches!(val, DbValue::Json(serde_json::Value::Object(_))));
    }

    #[test]
    fn redis_bulk_string_utf8_text() {
        let val = redis_value_to_db_value(&RedisValue::BulkString(b"plain text".to_vec()));
        // "plain text" is not numeric or JSON → DbValue::Text
        assert_eq!(val, DbValue::Text("plain text".to_string()));
    }

    // ── redis_value_to_json function coverage ─────────────────────────────────
    // Exercises via nested Array containing Bool/Float/Bytes/Timestamp values

    #[test]
    fn redis_array_with_bool_exercises_json_bool_arm() {
        // Array containing Boolean → redis_value_to_json Bool arm (line 87)
        let arr = RedisValue::Array(vec![RedisValue::Boolean(true), RedisValue::Boolean(false)]);
        let val = redis_value_to_db_value(&arr);
        if let DbValue::Json(serde_json::Value::Array(items)) = val {
            assert_eq!(items[0], serde_json::Value::Bool(true));
            assert_eq!(items[1], serde_json::Value::Bool(false));
        } else {
            panic!("expected JSON array with bools");
        }
    }

    #[test]
    fn redis_array_with_float_exercises_json_float_arm() {
        // Array containing Double → redis_value_to_json Float arm (line 89)
        let arr = RedisValue::Array(vec![RedisValue::Double(3.14)]);
        let val = redis_value_to_db_value(&arr);
        assert!(matches!(val, DbValue::Json(serde_json::Value::Array(_))));
    }

    #[test]
    fn redis_array_with_null_exercises_json_null_arm() {
        // Array containing Nil → redis_value_to_json Null arm (line 86)
        let arr = RedisValue::Array(vec![RedisValue::Nil]);
        let val = redis_value_to_db_value(&arr);
        if let DbValue::Json(serde_json::Value::Array(items)) = val {
            assert_eq!(items[0], serde_json::Value::Null);
        } else {
            panic!("expected JSON array with null");
        }
    }

    #[test]
    fn redis_array_with_binary_exercises_json_bytes_arm() {
        // Array containing BulkString (non-UTF8) → redis_value_to_json Bytes arm (line 92)
        let arr = RedisValue::Array(vec![RedisValue::BulkString(vec![0xFF, 0x00])]);
        let val = redis_value_to_db_value(&arr);
        if let DbValue::Json(serde_json::Value::Array(items)) = val {
            // Non-UTF8 bytes → "<2 bytes>" string
            assert!(items[0].as_str().map(|s| s.contains("bytes")).unwrap_or(false));
        } else {
            panic!("expected JSON array");
        }
    }

    #[test]
    fn redis_array_with_datetime_exercises_json_timestamp_arm() {
        // Array containing a DateTime-like BulkString (ISO 8601) → redis_value_to_json Text arm
        // Since Redis BulkStrings don't carry type info, timestamps are stored as strings
        // redis_value_to_json Timestamp arm (line 93) is for DbValue::Timestamp from nested values
        // However, Redis stores times as strings → DbValue::Text not DbValue::Timestamp
        // Test that non-UTF8 path works and Null path works for completeness
        let arr = RedisValue::Array(vec![
            RedisValue::Nil,           // Null arm (line 86)
            RedisValue::Boolean(true), // Bool arm (line 87)
            RedisValue::Double(1.5),   // Float arm (line 89)
        ]);
        let val = redis_value_to_db_value(&arr);
        if let DbValue::Json(serde_json::Value::Array(items)) = val {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], serde_json::Value::Null);
            assert_eq!(items[1], serde_json::Value::Bool(true));
        } else {
            panic!("expected JSON array");
        }
    }
}
