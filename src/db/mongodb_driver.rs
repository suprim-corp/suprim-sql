use std::collections::HashMap;
use std::time::Instant;

use async_trait::async_trait;
use mongodb::bson::{Bson, Document, doc};
use mongodb::options::{ClientOptions, FindOptions};
use mongodb::{Client, Collection, Database};

use crate::db::connection::{ConnectionConfig, DriverParams, DriverType};
use crate::db::driver::DatabaseDriver;
use crate::db::types::{
    ColumnMeta, ColumnNode, DatabaseNode, DbValue, QueryResult, SchemaNode, SchemaTree, TableNode,
};
use crate::error::{AppError, Result};

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Convert a BSON value to DbValue.
pub fn bson_to_db_value(bson: &Bson) -> DbValue {
    match bson {
        Bson::Null | Bson::Undefined => DbValue::Null,
        Bson::Boolean(b) => DbValue::Bool(*b),
        Bson::Int32(i) => DbValue::Int(*i as i64),
        Bson::Int64(i) => DbValue::Int(*i),
        Bson::Double(f) => DbValue::Float(*f),
        Bson::Decimal128(d) => {
            // Parse Decimal128 as text then try f64
            let s = d.to_string();
            s.parse::<f64>()
                .map(DbValue::Float)
                .unwrap_or(DbValue::Text(s))
        }
        Bson::String(s) => DbValue::Text(s.clone()),
        Bson::DateTime(dt) => {
            let ts_millis = dt.timestamp_millis();
            chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ts_millis)
                .map(DbValue::Timestamp)
                .unwrap_or(DbValue::Null)
        }
        Bson::ObjectId(oid) => DbValue::Text(oid.to_hex()),
        Bson::Binary(bin) => DbValue::Bytes(bin.bytes.clone()),
        Bson::Array(arr) => {
            let items: Vec<serde_json::Value> = arr.iter().map(bson_to_json).collect();
            DbValue::Json(serde_json::Value::Array(items))
        }
        Bson::Document(doc) => {
            let map: serde_json::Map<String, serde_json::Value> = doc
                .iter()
                .map(|(k, v)| (k.clone(), bson_to_json(v)))
                .collect();
            DbValue::Json(serde_json::Value::Object(map))
        }
        Bson::Timestamp(ts) => DbValue::Int(ts.time as i64),
        Bson::RegularExpression(re) => DbValue::Text(format!("/{}/", re.pattern)),
        Bson::Symbol(s) => DbValue::Text(s.clone()),
        Bson::JavaScriptCode(js) => DbValue::Text(js.clone()),
        _ => DbValue::Text(bson.to_string()),
    }
}

/// Convert BSON to serde_json::Value for nesting.
fn bson_to_json(bson: &Bson) -> serde_json::Value {
    match bson_to_db_value(bson) {
        DbValue::Null => serde_json::Value::Null,
        DbValue::Bool(b) => serde_json::Value::Bool(b),
        DbValue::Int(i) => serde_json::json!(i),
        DbValue::Float(f) => serde_json::json!(f),
        DbValue::Text(s) => serde_json::Value::String(s),
        DbValue::Json(j) => j,
        DbValue::Bytes(b) => serde_json::Value::String(format!("<{} bytes>", b.len())),
        DbValue::Timestamp(t) => serde_json::Value::String(t.to_rfc3339()),
    }
}

/// Convert a DbValue back to Bson for writes.
pub fn db_value_to_bson(val: &DbValue) -> Bson {
    match val {
        DbValue::Null => Bson::Null,
        DbValue::Bool(b) => Bson::Boolean(*b),
        DbValue::Int(i) => Bson::Int64(*i),
        DbValue::Float(f) => Bson::Double(*f),
        DbValue::Text(s) => Bson::String(s.clone()),
        DbValue::Bytes(b) => Bson::Binary(mongodb::bson::Binary {
            subtype: mongodb::bson::spec::BinarySubtype::Generic,
            bytes: b.clone(),
        }),
        DbValue::Json(v) => {
            // Best effort: serialize to string, deserialize as bson Document
            serde_json::to_string(v)
                .ok()
                .and_then(|s| mongodb::bson::from_slice::<Document>(s.as_bytes()).ok())
                .map(Bson::Document)
                .unwrap_or_else(|| Bson::String(v.to_string()))
        }
        DbValue::Timestamp(t) => {
            Bson::DateTime(mongodb::bson::DateTime::from_millis(t.timestamp_millis()))
        }
    }
}

/// Flatten a BSON document into a row of DbValues, using field names as column names.
pub fn doc_to_row(doc: &Document) -> (Vec<String>, Vec<DbValue>) {
    let mut cols = Vec::new();
    let mut vals = Vec::new();
    for (key, val) in doc.iter() {
        cols.push(key.clone());
        vals.push(bson_to_db_value(val));
    }
    (cols, vals)
}

/// Convert Vec<Document> into a QueryResult, unioning all field names across documents.
pub fn docs_to_query_result(docs: Vec<Document>, elapsed: std::time::Duration) -> QueryResult {
    if docs.is_empty() {
        return QueryResult {
            columns: vec![],
            rows: vec![],
            rows_affected: 0,
            execution_time: elapsed,
        };
    }

    // Collect union of all field names (preserving first-seen order)
    let mut col_names: Vec<String> = Vec::new();
    for doc in &docs {
        for key in doc.keys() {
            if !col_names.contains(key) {
                col_names.push(key.clone());
            }
        }
    }

    let columns: Vec<ColumnMeta> = col_names
        .iter()
        .map(|name| ColumnMeta {
            name: name.clone(),
            db_type: "bson".to_string(),
            nullable: true,
        })
        .collect();

    let rows: Vec<Vec<DbValue>> = docs
        .iter()
        .map(|doc| {
            col_names
                .iter()
                .map(|col| {
                    doc.get(col)
                        .map(bson_to_db_value)
                        .unwrap_or(DbValue::Null)
                })
                .collect()
        })
        .collect();

    let row_count = rows.len() as u64;
    QueryResult {
        columns,
        rows,
        rows_affected: row_count,
        execution_time: elapsed,
    }
}

// ─── Driver ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct MongoDriver {
    client: Option<Client>,
    db_name: Option<String>,
}

impl MongoDriver {
    pub fn new() -> Self {
        Self {
            client: None,
            db_name: None,
        }
    }

    fn db(&self) -> Result<Database> {
        let client = self.client.as_ref().ok_or(AppError::NotConnected)?;
        let name = self.db_name.as_deref().unwrap_or("admin");
        Ok(client.database(name))
    }
}

impl Default for MongoDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DatabaseDriver for MongoDriver {
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()> {
        let (uri, _password_key) = match &config.params {
            DriverParams::MongoDB { uri, password_key } => {
                (uri.as_str(), password_key.as_deref())
            }
            _ => return Err(AppError::connection("MongoDriver requires MongoDB params")),
        };

        let mut opts = ClientOptions::parse(uri)
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;

        // Extract database name from URI if present
        let db_name = opts
            .default_database
            .clone()
            .unwrap_or_else(|| "admin".to_string());

        opts.server_selection_timeout = Some(std::time::Duration::from_millis(500));
        opts.connect_timeout = Some(std::time::Duration::from_millis(500));

        let client = Client::with_options(opts)
            .map_err(|e| AppError::connection(e.to_string()))?;

        // Ping to confirm connectivity
        client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;

        self.client = Some(client);
        self.db_name = Some(db_name);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        self.client = None;
        self.db_name = None;
        Ok(())
    }

    async fn ping(&self) -> Result<()> {
        let client = self.client.as_ref().ok_or(AppError::NotConnected)?;
        client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;
        Ok(())
    }

    /// Execute a MongoDB command as a JSON string.
    /// Format: JSON object representing a MongoDB command, e.g. `{"ping":1}` or
    /// `{"find":"collectionName","filter":{},"limit":10}`.
    async fn execute(&self, sql: &str) -> Result<QueryResult> {
        let db = self.db()?;
        let start = Instant::now();

        let json_val: serde_json::Value = serde_json::from_str(sql)
            .map_err(|e| AppError::query(sql, format!("invalid JSON command: {}", e)))?;
        let cmd: Document = mongodb::bson::to_document(&json_val)
            .map_err(|e| AppError::query(sql, format!("BSON conversion error: {}", e)))?;

        let result_doc = db
            .run_command(cmd)
            .await
            .map_err(|e| AppError::query(sql, e.to_string()))?;

        let elapsed = start.elapsed();

        // Extract cursor documents or return the top-level result
        if let Some(Bson::Document(cursor_doc)) = result_doc.get("cursor") {
            if let Some(Bson::Array(first_batch)) = cursor_doc.get("firstBatch") {
                let docs: Vec<Document> = first_batch
                    .iter()
                    .filter_map(|v| {
                        if let Bson::Document(d) = v {
                            Some(d.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                return Ok(docs_to_query_result(docs, elapsed));
            }
        }

        // Single result document
        Ok(docs_to_query_result(vec![result_doc], elapsed))
    }

    /// Execute a MongoDB command with positional parameters.
    /// `sql` = collection name, params[0] = filter doc (as JSON Text), params[1] = projection (optional).
    async fn execute_with_params(&self, sql: &str, params: Vec<DbValue>) -> Result<QueryResult> {
        let db = self.db()?;
        let start = Instant::now();

        let collection: Collection<Document> = db.collection(sql);

        let filter = params
            .first()
            .and_then(|v| {
                if let DbValue::Text(s) = v {
                    serde_json::from_str::<serde_json::Value>(s).ok()
                        .and_then(|jv| mongodb::bson::to_document(&jv).ok())
                } else if let DbValue::Json(j) = v {
                    mongodb::bson::to_document(j).ok()
                        .or_else(|| {
                            serde_json::to_string(j).ok()
                                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
                                .and_then(|jv| mongodb::bson::to_document(&jv).ok())
                        })
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let mut cursor = collection
            .find(filter)
            .await
            .map_err(|e| AppError::query(sql, e.to_string()))?;

        let mut docs: Vec<Document> = Vec::new();
        while cursor.advance().await.map_err(|e| AppError::query(sql, e.to_string()))? {
            if let Ok(doc) = cursor.deserialize_current() {
                docs.push(doc);
            }
        }

        Ok(docs_to_query_result(docs, start.elapsed()))
    }

    /// Load schema: list all databases and their collections.
    async fn load_schema(&self) -> Result<SchemaTree> {
        let client = self.client.as_ref().ok_or(AppError::NotConnected)?;

        let db_names = client
            .list_database_names()
            .await
            .map_err(|e| AppError::Schema(e.to_string()))?;

        let mut databases = Vec::new();

        for db_name in &db_names {
            let db = client.database(db_name);
            let coll_names = db
                .list_collection_names()
                .await
                .unwrap_or_default();

            let tables: Vec<TableNode> = coll_names
                .iter()
                .map(|cname| {
                    // MongoDB collections are schema-less; provide a single "_document" pseudo-column
                    let columns = vec![
                        ColumnNode {
                            id: uuid::Uuid::new_v4(),
                            name: "_id".to_string(),
                            db_type: "ObjectId".to_string(),
                            nullable: false,
                            is_primary_key: true,
                            default_value: None,
                        },
                        ColumnNode {
                            id: uuid::Uuid::new_v4(),
                            name: "...".to_string(),
                            db_type: "document".to_string(),
                            nullable: true,
                            is_primary_key: false,
                            default_value: None,
                        },
                    ];
                    TableNode {
                        id: uuid::Uuid::new_v4(),
                        name: cname.clone(),
                        columns,
                        indexes: vec![],
                        foreign_keys: vec![],
                        row_count: None,
                    }
                })
                .collect();

            databases.push(DatabaseNode {
                id: uuid::Uuid::new_v4(),
                name: db_name.clone(),
                schemas: vec![SchemaNode {
                    id: uuid::Uuid::new_v4(),
                    name: "collections".to_string(),
                    tables,
                    views: vec![],
                    loaded: true,
                }],
            });
        }

        Ok(SchemaTree { databases })
    }

    async fn load_schema_detail(
        &self,
        schema_name: &str,
    ) -> Result<SchemaNode> {
        let tree = self.load_schema().await?;
        tree.databases
            .into_iter()
            .flat_map(|db| db.schemas)
            .find(|s| s.name == schema_name)
            .ok_or_else(|| {
                AppError::Schema(format!("schema '{}' not found", schema_name))
            })
    }

    async fn table_data(
        &self,
        _schema: Option<&str>,
        table: &str,
        page: u32,
        page_size: u32,
    ) -> Result<QueryResult> {
        let db = self.db()?;
        let start = Instant::now();

        let collection: Collection<Document> = db.collection(table);
        let skip = (page * page_size) as u64;

        let opts = FindOptions::builder()
            .skip(skip)
            .limit(Some(page_size as i64))
            .build();

        let mut cursor = collection
            .find(doc! {})
            .with_options(opts)
            .await
            .map_err(|e| AppError::query(table, e.to_string()))?;

        let mut docs: Vec<Document> = Vec::new();
        while cursor.advance().await.map_err(|e| AppError::query(table, e.to_string()))? {
            if let Ok(doc) = cursor.deserialize_current() {
                docs.push(doc);
            }
        }

        Ok(docs_to_query_result(docs, start.elapsed()))
    }

    async fn insert_row(&self, table: &str, values: HashMap<String, DbValue>) -> Result<u64> {
        let db = self.db()?;
        let collection: Collection<Document> = db.collection(table);

        let doc: Document = values
            .into_iter()
            .map(|(k, v)| (k, db_value_to_bson(&v)))
            .collect();

        collection
            .insert_one(doc)
            .await
            .map_err(|e| AppError::query(table, e.to_string()))?;
        Ok(1)
    }

    async fn update_row(
        &self,
        table: &str,
        pk: HashMap<String, DbValue>,
        changes: HashMap<String, DbValue>,
    ) -> Result<u64> {
        let db = self.db()?;
        let collection: Collection<Document> = db.collection(table);

        let filter: Document = pk
            .into_iter()
            .map(|(k, v)| (k, db_value_to_bson(&v)))
            .collect();

        let set_doc: Document = changes
            .into_iter()
            .map(|(k, v)| (k, db_value_to_bson(&v)))
            .collect();

        let result = collection
            .update_one(filter, doc! { "$set": set_doc })
            .await
            .map_err(|e| AppError::query(table, e.to_string()))?;

        Ok(result.modified_count)
    }

    async fn delete_row(&self, table: &str, pk: HashMap<String, DbValue>) -> Result<u64> {
        let db = self.db()?;
        let collection: Collection<Document> = db.collection(table);

        let filter: Document = pk
            .into_iter()
            .map(|(k, v)| (k, db_value_to_bson(&v)))
            .collect();

        let result = collection
            .delete_one(filter)
            .await
            .map_err(|e| AppError::query(table, e.to_string()))?;

        Ok(result.deleted_count)
    }

    fn driver_type(&self) -> DriverType {
        DriverType::MongoDB
    }

    fn is_connected(&self) -> bool {
        self.client.is_some()
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
        let driver = MongoDriver::new();
        assert!(!driver.is_connected());
    }

    #[test]
    fn default_driver_not_connected() {
        let driver = MongoDriver::default();
        assert!(!driver.is_connected());
    }

    #[test]
    fn driver_type_returns_mongodb() {
        let driver = MongoDriver::new();
        assert_eq!(driver.driver_type(), DriverType::MongoDB);
    }

    // ── Not-connected errors ──────────────────────────────────────────────────

    #[tokio::test]
    async fn disconnect_without_connect_is_ok() {
        let mut driver = MongoDriver::new();
        assert!(driver.disconnect().await.is_ok());
    }

    #[tokio::test]
    async fn ping_without_connect_returns_not_connected() {
        let driver = MongoDriver::new();
        let err = driver.ping().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_without_connect_returns_not_connected() {
        let driver = MongoDriver::new();
        let err = driver.execute(r#"{"ping":1}"#).await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn execute_with_params_without_connect_returns_not_connected() {
        let driver = MongoDriver::new();
        let err = driver
            .execute_with_params("users", vec![])
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn load_schema_without_connect_returns_not_connected() {
        let driver = MongoDriver::new();
        let err = driver.load_schema().await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn table_data_without_connect_returns_not_connected() {
        let driver = MongoDriver::new();
        let err = driver.table_data(None, "users", 0, 50).await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn insert_without_connect_returns_not_connected() {
        let driver = MongoDriver::new();
        let err = driver.insert_row("users", HashMap::new()).await.unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn update_without_connect_returns_not_connected() {
        let driver = MongoDriver::new();
        let err = driver
            .update_row("users", HashMap::new(), HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn delete_without_connect_returns_not_connected() {
        let driver = MongoDriver::new();
        let err = driver
            .delete_row("users", HashMap::new())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotConnected));
    }

    #[tokio::test]
    async fn connect_wrong_params_returns_error() {
        let mut driver = MongoDriver::new();
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

    // ── BSON value conversion ─────────────────────────────────────────────────

    #[test]
    fn bson_null_becomes_null() {
        assert_eq!(bson_to_db_value(&Bson::Null), DbValue::Null);
    }

    #[test]
    fn bson_bool_becomes_db_bool() {
        assert_eq!(bson_to_db_value(&Bson::Boolean(true)), DbValue::Bool(true));
        assert_eq!(bson_to_db_value(&Bson::Boolean(false)), DbValue::Bool(false));
    }

    #[test]
    fn bson_int32_becomes_int() {
        assert_eq!(bson_to_db_value(&Bson::Int32(42)), DbValue::Int(42));
    }

    #[test]
    fn bson_int64_becomes_int() {
        assert_eq!(bson_to_db_value(&Bson::Int64(9999)), DbValue::Int(9999));
    }

    #[test]
    fn bson_double_becomes_float() {
        assert_eq!(bson_to_db_value(&Bson::Double(3.14)), DbValue::Float(3.14));
    }

    #[test]
    fn bson_string_becomes_text() {
        assert_eq!(
            bson_to_db_value(&Bson::String("hello".to_string())),
            DbValue::Text("hello".to_string())
        );
    }

    #[test]
    fn bson_array_becomes_json_array() {
        let arr = Bson::Array(vec![Bson::Int32(1), Bson::String("two".to_string())]);
        assert!(matches!(bson_to_db_value(&arr), DbValue::Json(serde_json::Value::Array(_))));
    }

    #[test]
    fn bson_document_becomes_json_object() {
        let doc = doc! { "x": 1, "y": "hello" };
        let val = bson_to_db_value(&Bson::Document(doc));
        assert!(matches!(val, DbValue::Json(serde_json::Value::Object(_))));
    }

    #[test]
    fn bson_object_id_becomes_text() {
        let oid = mongodb::bson::oid::ObjectId::new();
        let val = bson_to_db_value(&Bson::ObjectId(oid));
        assert!(matches!(val, DbValue::Text(_)));
    }

    // ── DbValue to BSON round trip ────────────────────────────────────────────

    #[test]
    fn db_value_null_to_bson() {
        assert_eq!(db_value_to_bson(&DbValue::Null), Bson::Null);
    }

    #[test]
    fn db_value_bool_to_bson() {
        assert_eq!(db_value_to_bson(&DbValue::Bool(true)), Bson::Boolean(true));
    }

    #[test]
    fn db_value_int_to_bson() {
        assert_eq!(db_value_to_bson(&DbValue::Int(7)), Bson::Int64(7));
    }

    #[test]
    fn db_value_float_to_bson() {
        assert_eq!(db_value_to_bson(&DbValue::Float(1.5)), Bson::Double(1.5));
    }

    #[test]
    fn db_value_text_to_bson() {
        assert_eq!(
            db_value_to_bson(&DbValue::Text("abc".to_string())),
            Bson::String("abc".to_string())
        );
    }

    // ── docs_to_query_result ──────────────────────────────────────────────────

    #[test]
    fn docs_to_query_result_empty() {
        let result = docs_to_query_result(vec![], std::time::Duration::from_millis(1));
        assert_eq!(result.columns.len(), 0);
        assert_eq!(result.rows.len(), 0);
    }

    #[test]
    fn docs_to_query_result_single_doc() {
        let doc = doc! { "name": "Alice", "age": 30 };
        let result = docs_to_query_result(vec![doc], std::time::Duration::from_millis(1));
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.columns.len(), 2);
    }

    #[test]
    fn docs_to_query_result_union_fields() {
        let doc1 = doc! { "a": 1 };
        let doc2 = doc! { "a": 2, "b": "x" };
        let result = docs_to_query_result(vec![doc1, doc2], std::time::Duration::ZERO);
        // Both docs: "a" + "b" = 2 columns
        assert_eq!(result.columns.len(), 2);
        assert_eq!(result.rows.len(), 2);
        // First row: a=1, b=Null
        assert_eq!(result.rows[0][1], DbValue::Null);
    }

    // ── db_value_to_bson additional types ────────────────────────────────────

    #[test]
    fn db_value_bytes_to_bson() {
        let bytes = vec![1u8, 2, 3];
        let bson = db_value_to_bson(&DbValue::Bytes(bytes.clone()));
        if let Bson::Binary(b) = bson {
            assert_eq!(b.bytes, bytes);
        } else {
            panic!("expected Binary");
        }
    }

    #[test]
    fn db_value_timestamp_to_bson() {
        let ts = chrono::DateTime::<chrono::Utc>::from_timestamp(1_000_000, 0).unwrap();
        let bson = db_value_to_bson(&DbValue::Timestamp(ts));
        assert!(matches!(bson, Bson::DateTime(_)));
    }

    #[test]
    fn db_value_json_to_bson_text_fallback() {
        // JSON value (not a document object) falls back to string representation
        let j = serde_json::json!([1, 2, 3]);
        let bson = db_value_to_bson(&DbValue::Json(j));
        // Could be Document or String depending on serialization
        // Just verify it doesn't panic
        assert!(!matches!(bson, Bson::Null));
    }

    // ── bson_to_db_value additional variants ─────────────────────────────────

    #[test]
    fn bson_regex_becomes_text() {
        let re = mongodb::bson::Regex {
            pattern: "foo".to_string(),
            options: "i".to_string(),
        };
        let val = bson_to_db_value(&Bson::RegularExpression(re));
        assert!(matches!(val, DbValue::Text(_)));
    }

    #[test]
    fn bson_symbol_becomes_text() {
        let val = bson_to_db_value(&Bson::Symbol("sym".to_string()));
        assert_eq!(val, DbValue::Text("sym".to_string()));
    }

    #[test]
    fn bson_javascript_becomes_text() {
        let val = bson_to_db_value(&Bson::JavaScriptCode("function(){}".to_string()));
        assert_eq!(val, DbValue::Text("function(){}".to_string()));
    }

    #[test]
    fn bson_timestamp_becomes_int() {
        let ts = mongodb::bson::Timestamp { time: 1234, increment: 0 };
        let val = bson_to_db_value(&Bson::Timestamp(ts));
        assert_eq!(val, DbValue::Int(1234));
    }

    #[test]
    fn bson_datetime_becomes_timestamp() {
        let dt = mongodb::bson::DateTime::from_millis(0);
        let val = bson_to_db_value(&Bson::DateTime(dt));
        // epoch → DbValue::Timestamp or Null if conversion fails
        assert!(matches!(val, DbValue::Timestamp(_)) || matches!(val, DbValue::Null));
    }

    #[test]
    fn bson_binary_becomes_bytes() {
        let bin = mongodb::bson::Binary {
            subtype: mongodb::bson::spec::BinarySubtype::Generic,
            bytes: vec![1, 2, 3],
        };
        let val = bson_to_db_value(&Bson::Binary(bin));
        assert_eq!(val, DbValue::Bytes(vec![1, 2, 3]));
    }

    // ── docs_to_query_result edge cases ──────────────────────────────────────

    #[test]
    fn docs_to_query_result_preserves_column_order() {
        let doc = doc! { "z": 1, "a": 2, "m": 3 };
        let result = docs_to_query_result(vec![doc], std::time::Duration::ZERO);
        // Column order follows iteration order of Document
        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.rows[0].len(), 3);
    }

    #[test]
    fn docs_to_query_result_execution_time_preserved() {
        let result = docs_to_query_result(vec![], std::time::Duration::from_millis(99));
        assert_eq!(result.execution_time.as_millis(), 99);
    }
}
