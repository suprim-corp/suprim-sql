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

Mọi DB driver đều map native types về `DbValue`. UI chỉ biết đến `DbValue`.

### `QueryResult`

```
QueryResult {
    columns: Vec<ColumnMeta>      // tên + type hint
    rows: Vec<Vec<DbValue>>       // data
    rows_affected: u64            // cho INSERT/UPDATE/DELETE
    execution_time: Duration
}

ColumnMeta {
    name: String
    db_type: String               // raw type string từ DB
    nullable: bool
}
```

### `SchemaTree` — sidebar model

```
SchemaTree
└── Vec<DatabaseNode>
    └── schemas: Vec<SchemaNode>
        └── tables: Vec<TableNode>
            ├── columns: Vec<ColumnNode>
            ├── indexes: Vec<IndexNode>
            └── foreign_keys: Vec<FkNode>
```

Mỗi node có `id: Uuid` để UI có thể track expand/collapse state độc lập.

### `ConnectionConfig` — serializable, per-driver

```
ConnectionConfig {
    id: Uuid
    name: String                  // display name
    driver: DriverType            // enum: Postgres, MySQL, SQLite, ...
    params: DriverParams          // enum variant per driver
    ssh: Option<SshConfig>
    tls: TlsConfig
    created_at: DateTime<Utc>
    last_used: Option<DateTime<Utc>>
}

DriverParams
├── Postgres { host, port, database, user, password_key }
├── MySQL    { host, port, database, user, password_key }
├── SQLite   { path: PathBuf }
├── Redis    { host, port, db_index, password_key }
├── MongoDB  { uri, password_key }
└── Mssql    { host, port, database, user, password_key }
```

`password_key` là key để lookup từ OS keychain — không bao giờ lưu password plaintext vào disk.

---

## 2. Database Layer

### `DatabaseDriver` trait

```rust
#[async_trait]
trait DatabaseDriver: Send + Sync {
    async fn connect(&mut self, config: &ConnectionConfig) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn ping(&self) -> Result<()>;

    // Query
    async fn execute(&self, sql: &str) -> Result<QueryResult>;
    async fn execute_with_params(&self, sql: &str, params: Vec<DbValue>) -> Result<QueryResult>;

    // Schema
    async fn load_schema(&self) -> Result<SchemaTree>;
    async fn table_data(&self, table: &str, page: u32, page_size: u32) -> Result<QueryResult>;

    // Mutations (inline edit)
    async fn insert_row(&self, table: &str, values: HashMap<String, DbValue>) -> Result<u64>;
    async fn update_row(&self, table: &str, pk: HashMap<String, DbValue>, changes: HashMap<String, DbValue>) -> Result<u64>;
    async fn delete_row(&self, table: &str, pk: HashMap<String, DbValue>) -> Result<u64>;

    fn driver_type(&self) -> DriverType;
}
```

`DbFactory::create(config) -> Box<dyn DatabaseDriver>` — runtime dispatch.

### Async communication: UI ↔ DB worker

UI thread không bao giờ `.await` trực tiếp. Dùng command/response channel:

```
DbCommand (UI → worker)
├── Connect(ConnectionConfig)
├── Execute { conn_id, sql, tab_id }
├── LoadSchema { conn_id }
├── LoadTableData { conn_id, table, page }
├── InsertRow / UpdateRow / DeleteRow
└── Disconnect(conn_id)

DbEvent (worker → UI)
├── Connected { conn_id, schema: SchemaTree }
├── QueryResult { tab_id, result: QueryResult }
├── SchemaLoaded { conn_id, schema: SchemaTree }
├── Error { tab_id, error: AppError }
└── Disconnected(conn_id)
```

`DbWorker` chạy trong `tokio::spawn`, nhận `mpsc::Receiver<DbCommand>`, gửi `mpsc::Sender<DbEvent>` về UI. UI poll events mỗi frame egui.

---

## 3. App State

```
AppState {
    // Connections
    connections: HashMap<Uuid, ActiveConnection>
    saved_configs: Vec<ConnectionConfig>

    // UI
    open_tabs: Vec<Tab>
    active_tab: usize
    sidebar: SidebarState

    // Channels
    cmd_tx: mpsc::Sender<DbCommand>
    event_rx: mpsc::Receiver<DbEvent>

    // Storage
    config_store: ConfigStore
    history: QueryHistory
    workspace: Workspace
}

ActiveConnection {
    config: ConnectionConfig
    status: ConnectionStatus    // Connecting | Connected | Error(String)
    schema: Option<SchemaTree>
}
```

---

## 4. UI Component Model

### Tab system

```
Tab (trait)
├── id: Uuid
├── title: String
├── conn_id: Option<Uuid>
├── render(&mut self, ui: &mut egui::Ui, state: &mut AppState)
└── on_close(&mut self)

Implementations:
├── SqlEditorTab     { editor_content, result, is_running, query_id }
├── TableViewerTab   { table, page, rows, total_count }
├── TableEditorTab   { table, rows, pending_changes }
├── DiagramTab       { schema_snapshot }
├── RedisTab         { scan_cursor, keys, selected_key, value }
├── RedisPubSubTab   { channel, messages }
└── MongoTab         { collection, filter, documents }
```

### Sidebar state

```
SidebarState {
    expanded: HashSet<Uuid>       // track which nodes expanded
    selected: Option<Uuid>        // selected node
    search_query: String
    width: f32
}
```

Sidebar click vào table → gửi `DbCommand::LoadTableData` → mở `TableViewerTab`.

---

## 5. Storage

**Config file** (`~/.config/suprim-sql/connections.toml`):
- Lưu `ConnectionConfig` không có password
- Mã hóa AES-256-GCM với key từ OS keychain

**Keychain**:
- Service name: `suprim-sql`
- Key format: `conn-{uuid}`
- Value: password plaintext (keychain tự encrypt ở OS level)

**Query history** (SQLite local `~/.local/share/suprim-sql/history.db`):
```sql
CREATE TABLE history (
    id          INTEGER PRIMARY KEY,
    conn_id     TEXT,
    sql         TEXT,
    executed_at DATETIME,
    duration_ms INTEGER,
    error       TEXT
);
```

**Workspace** (`~/.config/suprim-sql/workspace.toml`):
- Open tabs state, sidebar expand state, window size/position

---

## 6. Error Handling

```
AppError
├── Connection(String)
├── Query { sql: String, message: String }
├── Schema(String)
├── Io(std::io::Error)
├── Crypto(String)
├── Config(String)
└── Driver { driver: DriverType, source: Box<dyn std::error::Error> }
```

Dùng `thiserror` để derive. Tất cả `Result<T>` trong codebase đều là `Result<T, AppError>`.

---

## 7. Testing Strategy

### Layer 1: Unit tests (không cần DB)

- `DbValue` serialization/deserialization
- `SqlBuilder` — generate SQL đúng với các edge case
- `ConnectionConfig` encrypt/decrypt
- `SchemaTree` traversal/search
- `ColumnMeta` type mapping logic

### Layer 2: Integration tests (testcontainers)

Mỗi driver có integration test riêng, spin up container qua `testcontainers-rs`:

```
tests/
├── postgres_driver_test.rs    # image: postgres:15
├── mysql_driver_test.rs       # image: mysql:8
├── redis_driver_test.rs       # image: redis:7
└── mongodb_driver_test.rs     # image: mongo:7
```

Test cases chung cho mọi SQL driver:
- `test_connect_ok / test_connect_wrong_password`
- `test_execute_select`
- `test_execute_insert_update_delete`
- `test_load_schema` — verify tables/columns đúng
- `test_table_data_pagination`
- `test_transaction_rollback`

SQLite không cần container — dùng `:memory:`.

### Layer 3: Channel tests (mock driver)

- Mock `DatabaseDriver` với `mockall`
- Test `DbWorker` nhận đúng command, gửi đúng event
- Test error propagation qua channel

### Layer 4: UI snapshot tests (tương lai)

- `egui_kittest` hoặc manual egui `TestHarness`
- Verify sidebar render, tab render không panic

### CI matrix

```yaml
test:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]

# Integration tests chỉ chạy trên Linux (Docker available)
integration-test:
  runs-on: ubuntu-latest
  services:
    postgres: { image: postgres:15 }
    mysql:    { image: mysql:8 }
    redis:    { image: redis:7 }
    mongo:    { image: mongo:7 }
```

---

## 8. Implementation Phases

```
Phase 1: Foundation
  ├── Cargo.toml setup
  ├── core types (DbValue, QueryResult, SchemaTree, AppError)
  └── ConnectionConfig + storage/keychain

Phase 2: DB Drivers
  ├── SQLite (no container, validate interface)
  ├── PostgreSQL
  ├── MySQL
  └── Redis / MongoDB / MSSQL

Phase 3: Async Worker
  └── DbWorker + channel protocol

Phase 4: UI
  ├── App skeleton (egui window)
  ├── Sidebar
  ├── Tab system
  └── SQL editor + result grid

Phase 5: Features
  ├── SSH tunnel
  ├── AI assistant
  ├── Export/import
  └── ERD diagram
```
