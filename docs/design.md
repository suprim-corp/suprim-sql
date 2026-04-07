# Design Document — suprim-sql

## 1. Core Data Model

### `DbValue` — universal value type

```
DbValue
├── Null
├── Bool(bool)
├── Int(i64)
├── Float(f64)
├── Text(String)
├── Bytes(Vec<u8>)
├── Json(serde_json::Value)
└── Timestamp(chrono::DateTime<Utc>)
```

All DB drivers map native types to `DbValue`. UI only works with `DbValue`.

Traits: `Debug, Clone, PartialEq, Serialize, Deserialize`.
Helper methods: `is_null()`, `display()` (UI rendering), `Display` trait impl.

### `ColumnMeta`

```rust
ColumnMeta {
    name: String,
    db_type: String,      // raw type string from DB (e.g. "int4", "varchar")
    nullable: bool,
}
```

### `QueryResult`

```rust
QueryResult {
    columns: Vec<ColumnMeta>,
    rows: Vec<Vec<DbValue>>,
    rows_affected: u64,           // for INSERT/UPDATE/DELETE
    execution_time: Duration,
    total_count: Option<u64>,     // total rows before LIMIT — for pagination
}
```

`total_count` is set by `table_data()` queries (runs `COUNT(*)` in same READ ONLY transaction). `None` for raw SQL execution via `execute()`.

Helper methods: `empty()`, `row_count()`, `column_count()`.

### `SchemaTree` — sidebar model (lazy 3-level hierarchy)

```
SchemaTree
└── databases: Vec<DatabaseNode>
    └── schemas: Vec<SchemaNode>
        ├── tables: Vec<TableNode>
        │   ├── columns: Vec<ColumnNode>
        │   ├── indexes: Vec<IndexNode>
        │   └── foreign_keys: Vec<ForeignKeyNode>
        ├── views: Vec<ViewNode>
        │   └── columns: Vec<ColumnNode>
        ├── materialized_views: Vec<ViewNode>
        └── sequences: Vec<SequenceNode>
```

Schema loading is lazy — 3 steps:
1. `list_databases()` → populate `DatabaseNode.name` entries
2. `list_schemas(database)` → populate `SchemaNode.name` entries (on database click)
3. `load_schema_detail(database, schema)` → fill tables/views/columns/indexes/FKs (on schema click)

`SchemaNode.loaded: bool` tracks whether detail has been fetched.

Node types:
- `DatabaseNode { id: Uuid, name, schemas }`
- `SchemaNode { id: Uuid, name, tables, views, materialized_views, sequences, loaded }`
- `TableNode { id: Uuid, name, columns, indexes, foreign_keys, row_count: Option<u64> }`
- `ViewNode { id: Uuid, name, columns }`
- `ColumnNode { id: Uuid, name, db_type, nullable, is_primary_key, default_value: Option<String> }`
- `IndexNode { id: Uuid, name, columns: Vec<String>, is_unique }`
- `ForeignKeyNode { id: Uuid, name, columns, ref_table, ref_columns }`
- `SequenceNode { id: Uuid, name }`

Each node has `id: Uuid` for UI expand/collapse state tracking.

### `ConnectionConfig` — serializable, per-driver

```rust
ConnectionConfig {
    id: Uuid,
    name: String,                 // display name
    params: DriverParams,         // enum variant per driver
    ssh: Option<SshConfig>,
    tls: TlsConfig,               // #[serde(default)]
    created_at: DateTime<Utc>,
    last_used: Option<DateTime<Utc>>,
    visible_databases: Option<Vec<String>>,  // filter sidebar databases
}
```

`visible_databases`: `None` or empty = show all databases. Set via database picker popup in sidebar.

```
DriverType: Sqlite | Postgres | Mysql | Redis | MongoDB | Mssql

DriverParams (tagged enum, serde tag = "type")
├── Sqlite   { path: PathBuf }
├── Postgres { host, port, database, user, password_key }
├── Mysql    { host, port, database, user, password_key }
├── Redis    { host, port, db_index: u8, password_key: Option }
├── MongoDB  { uri, password_key: Option }
└── Mssql    { host, port, database, user, password_key }

SshConfig { host, port, user, key_path: Option<PathBuf>, password_key: Option }
TlsConfig { enabled, verify_cert, ca_cert_path, client_cert_path, client_key_path }
```

`password_key` is a key for OS keychain lookup — never stored as plaintext.

---

## 2. Database Layer

### `DatabaseDriver` trait

```rust
#[async_trait]
trait DatabaseDriver: Send + Sync + Debug {
    // Connection lifecycle
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn ping(&self) -> Result<()>;

    // Query execution
    async fn execute(&self, sql: &str) -> Result<QueryResult>;
    async fn execute_with_params(&self, sql: &str, params: Vec<DbValue>) -> Result<QueryResult>;

    // Schema — lazy 3-level hierarchy
    async fn list_databases(&self) -> Result<Vec<String>>;
    async fn list_schemas(&self, database: &str) -> Result<Vec<String>>;
    async fn load_schema_detail(&self, database: &str, schema_name: &str) -> Result<SchemaNode>;

    // Table data with filtering
    async fn table_data(
        &self,
        database: Option<&str>,
        schema: Option<&str>,
        table: &str,
        page: u32,
        page_size: u32,
        where_clause: Option<&str>,
        order_clause: Option<&str>,
    ) -> Result<QueryResult>;

    // Mutations (inline table editor)
    async fn insert_row(&self, table: &str, values: HashMap<String, DbValue>) -> Result<u64>;
    async fn update_row(&self, table: &str, pk: HashMap<String, DbValue>, changes: HashMap<String, DbValue>) -> Result<u64>;
    async fn delete_row(&self, table: &str, pk: HashMap<String, DbValue>) -> Result<u64>;

    // Metadata
    fn driver_type(&self) -> DriverType;
    fn is_connected(&self) -> bool;
}
```

`DbFactory::create(config) -> Box<dyn DatabaseDriver>` — runtime dispatch. Currently only Postgres active; other 5 return "not yet available".

### PostgreSQL Driver — key implementation details

- **Per-database pool cache**: `db_pools: Mutex<HashMap<String, PgPool>>` — creates/caches pool per database since Postgres doesn't support cross-database queries
- **`pool_for_db(database)`**: Returns cached pool or creates new one from base `PgConnectOptions`
- **READ ONLY transactions**: `table_data()` wraps queries in `BEGIN; SET TRANSACTION READ ONLY; ... COMMIT;` — prevents SQL injection via user-provided WHERE/ORDER BY
- **COUNT in same transaction**: `table_data()` runs `SELECT COUNT(*)` alongside `SELECT` data query in same READ ONLY transaction, populates `total_count`
- **Submodules**: `connection_url.rs` (URL builder), `type_mapping.rs` (Postgres types → DbValue), `schema_loader.rs` (lazy hierarchy queries), `queries.rs` (table_data + read-only tx)

### Async communication: UI ↔ DB worker

UI thread never calls `.await` directly. Uses command/response channels:

```
DbCommand (UI → worker) — 11 variants
├── Connect { config }
├── Disconnect { conn_id }
├── Execute { conn_id, tab_id, sql }
├── ListDatabases { conn_id }
├── ListSchemas { conn_id, database }
├── LoadSchemaDetail { conn_id, database, schema_name }
├── LoadTableData { conn_id, tab_id, database?, schema?, table, page, page_size, where?, order? }
├── InsertRow { conn_id, tab_id, table, values }
├── UpdateRow { conn_id, tab_id, table, pk, changes }
├── DeleteRow { conn_id, tab_id, table, pk }
└── Shutdown

DbEvent (worker → UI) — 8 variants
├── Connected { conn_id, databases: Vec<String> }
├── Disconnected { conn_id }
├── QueryResult { tab_id, result }
├── DatabasesListed { conn_id, databases }
├── SchemasListed { conn_id, database, schemas }
├── SchemaDetailLoaded { conn_id, database, schema_name, schema_node }
├── RowMutated { tab_id, rows_affected }
└── Error { tab_id?, conn_id?, message }
```

`DbWorker` runs inside `tokio::spawn`, receives `mpsc::Receiver<DbCommand>`, sends `mpsc::Sender<DbEvent>` back to UI. Owns `HashMap<Uuid, Box<dyn DatabaseDriver>>` for all active connections. UI polls events each frame.

---

## 3. App State

```rust
App {
    // Channels
    cmd_tx: mpsc::Sender<DbCommand>,
    event_rx: mpsc::Receiver<DbEvent>,

    // UI components (owned, not trait objects)
    sidebar: Sidebar,
    tab_manager: TabManager,
    statusbar: StatusBar,

    // Modal
    connection_dialog: Option<ConnectionDialog>,

    // State
    status: String,
    config: AppConfig,              // persisted connections list
}
```

`App` implements `eframe::App` with:
- `fn update()`: renders UI (sidebar, tabs, statusbar, connection dialog)
- Process events from `event_rx` each frame
- Phosphor icon font registered in `CreationContext`
- Auto-reconnects all saved connections on startup

### Sidebar

```rust
Sidebar {
    connections: Vec<ConnectionEntry>,    // active connections with schema trees
    expanded: HashSet<Uuid>,              // which nodes are expanded
    // ... database picker state, schema_detail_requested debounce
}

ConnectionEntry {
    config: ConnectionConfig,
    status: ConnectionStatus,
    schema_tree: SchemaTree,
}
```

Features: lazy-loading schema tree, database filter/picker popup, debounced detail requests.

### TabManager

```rust
TabManager {
    tabs: Vec<TabEntry>,
    active_tab: Option<Uuid>,
}

TabEntry {
    id: Uuid,
    kind: TabKind,               // SqlEditor | TableViewer
    conn_id: Uuid,
    title: String,
}
```

`TabKind::SqlEditor` — code editor + result grid.
`TabKind::TableViewer` — table data with WHERE/ORDER BY filter bar, cell editor popup (with JSON syntax highlighting), pagination (page X / Y, N rows).

---

## 4. Storage

**Config file** (`~/.config/suprim-sql/connections.toml`):
- Stores `Vec<ConnectionConfig>` serialized as TOML
- No plaintext passwords — uses `password_key` for keychain lookup
- `AppConfig::load()` / `AppConfig::save()`

**Planned (not yet implemented):**
- Keychain: `keyring-rs` service `suprim-sql`, key `conn-{uuid}`
- Query history: local SQLite `~/.local/share/suprim-sql/history.db`
- Workspace state: open tabs, sidebar state, window size

---

## 5. Error Handling

```
AppError (thiserror)
├── Connection(String)
├── Query { sql, message }
├── Schema(String)
├── Io(std::io::Error)              # #[from]
├── Crypto(String)
├── Config(String)
├── Keychain(String)
├── Driver { driver: DriverType, source: Box<dyn Error + Send + Sync> }
├── NotConnected
└── Cancelled
```

`pub type Result<T> = std::result::Result<T, AppError>;`

Convenience constructors: `AppError::driver()`, `connection()`, `query()`, `config()`, `crypto()`.

`AppError` is `Send + Sync`.

---

## 6. Testing Strategy

### Unit tests (in-source `#[cfg(test)]`)

- `DbValue` display, serialization roundtrip, `is_null()`
- `ColumnMeta`, `QueryResult` helpers
- `ConnectionConfig` TOML serde roundtrip
- `DriverType` display, `DriverParams::driver_type()`
- All `AppError` variants display formatting
- `SchemaTree::default()`

### Integration tests (testcontainers)

Each driver has integration tests spinning up real containers:

```
tests/
├── postgres_driver_test.rs   # 14 tests (testcontainers postgres:15)
├── sqlite_driver_test.rs     # 12 tests (:memory:, --test-threads=1)
├── mysql_driver_test.rs      # 11 tests (testcontainers mysql:8)
├── redis_driver_test.rs      # 10 tests (200ms delay, --test-threads=1)
├── mongodb_driver_test.rs    # 10 tests (--test-threads=1)
└── mssql_driver_test.rs      # 10 tests (crashes on Apple Silicon — excluded from coverage)
```

Test cases per driver: connect, ping, execute DDL/DML/SELECT, list_databases, list_schemas, load_schema_detail, table_data with pagination/WHERE/ORDER BY, type mapping.

### Coverage

Target: 90% unit test coverage (excluding mssql.rs and main.rs).
Command: `make coverage` (cargo tarpaulin).

---

## 7. Implementation Phases

```
Phase 1: Foundation ✓
  ├── Cargo.toml setup
  ├── Core types (DbValue, QueryResult, SchemaTree, AppError)
  ├── ConnectionConfig + TOML storage
  └── DbWorker async channel protocol

Phase 2: DB Drivers ✓ (written, Postgres active)
  ├── PostgreSQL (active, split into submodules)
  ├── SQLite (written, commented out)
  ├── MySQL (written, commented out)
  ├── Redis (written, commented out)
  ├── MongoDB (written, commented out)
  └── MSSQL (written, commented out)

Phase 3: UI ✓
  ├── eframe App skeleton with Phosphor fonts
  ├── Sidebar with lazy 3-level schema loading
  ├── Tab system (SQL editor + table viewer)
  ├── Result grid with virtual scrolling + display cache
  ├── Connection dialog (supports all 6 DB types)
  ├── Cell editor with JSON syntax highlighting
  └── Pagination with total count

Phase 4: Polish (in progress)
  ├── Coverage target (currently ~86%, target 90%)
  ├── Theme-adaptive colors (no hardcoded colors)
  └── Database filter/picker

Phase 5: Features (planned)
  ├── Re-enable 5 commented-out drivers
  ├── SSH tunnel (russh)
  ├── Keychain credential storage (keyring-rs)
  ├── AI assistant (async-openai)
  ├── Export/import (CSV, JSON, Excel)
  ├── ERD diagram
  ├── SQL autocomplete & formatter
  └── Query history
```
