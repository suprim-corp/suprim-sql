# suprim-sql

A simple, cross-platform SQL client written in Rust. No Electron. No JVM. No bloat.

## Current Status

**v0.1** — PostgreSQL fully working. 5 other drivers (SQLite, MySQL, Redis, MongoDB, MSSQL) are written and tested but commented out for faster initial shipping.

## Features (Implemented)

- **PostgreSQL support**: Full driver with connection pooling, lazy schema loading, query execution
- **SQL Editor**: Syntax highlighting via `egui_code_editor`, run queries with results in data grid
- **Table Viewer**: Browse table data with pagination, WHERE/ORDER BY filter bar, total row count
- **Schema Browser**: Lazy 3-level sidebar (databases -> schemas -> tables/views/columns/indexes/FKs/sequences)
- **Connection Manager**: Save/restore connections to `~/.config/suprim-sql/connections.toml`
- **Database Filter**: Pick which databases to show per connection (visible_databases)
- **Multi-tab**: SQL editor tabs + table viewer tabs, open/close/switch
- **Cell Inspector**: Click cell to view/edit, JSON syntax highlighting for JSON columns
- **Virtual Scrolling**: Only visible rows rendered — handles large result sets
- **Theme-adaptive UI**: All colors derived from system theme via `ui.visuals()`
- **Phosphor Icons**: 1531 icons via `egui-phosphor`
- **Cross-platform**: macOS (Metal), Linux (OpenGL/Wayland/X11), Windows (DirectX)

## Features (Planned)

- SQLite, MySQL, Redis, MongoDB, MSSQL (drivers written, not yet active)
- SSH tunneling (`russh`)
- TLS/SSL connections
- Encrypted credentials via OS keychain (`keyring-rs`)
- AI assistant for SQL writing (`async-openai`)
- Export/Import: CSV, JSON, Excel
- ERD diagram
- SQL autocomplete & formatter
- Query history
- Inline table editing (INSERT/UPDATE/DELETE with SQL preview)

## Tech Stack

| Layer | Library | Status |
|---|---|---|
| UI | [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) 0.34 + [egui](https://github.com/emilk/egui) 0.34 | Active |
| Icons | [egui-phosphor](https://github.com/amPerl/egui-phosphor) 0.12.0 | Active |
| Code editor | [egui_code_editor](https://github.com/nickeisenberg/egui_code_editor) 0.2 | Active |
| Async runtime | [tokio](https://github.com/tokio-rs/tokio) | Active |
| PostgreSQL / MySQL / SQLite | [sqlx](https://github.com/launchbadge/sqlx) 0.9.0-alpha.1 | Postgres active |
| MongoDB | [mongodb](https://github.com/mongodb/mongo-rust-driver) 3 | Written, inactive |
| Redis | [redis-rs](https://github.com/redis-rs/redis-rs) 1 | Written, inactive |
| MSSQL | [tiberius](https://github.com/prisma/tiberius) 0.11 | Written, inactive |
| SSH Tunnel | [russh](https://github.com/warp-tech/russh) 0.60 | Not yet used |
| File Dialogs | [rfd](https://github.com/PolyMeilex/rfd) 0.17 | Not yet used |
| Credentials | [keyring-rs](https://github.com/hwchen/keyring-rs) 4.0.0-rc.3 | Not yet used |
| AI | [async-openai](https://github.com/64bit/async-openai) 0.34 | Not yet used |
| Serialization | [serde](https://serde.rs) + [toml](https://github.com/toml-rs/toml) | Active |
| Error handling | [thiserror](https://github.com/dtolnay/thiserror) | Active |

## Render Backend

- **macOS** — Metal
- **Linux** — OpenGL / Wayland / X11
- **Windows** — DirectX

## Project Structure

```
suprim-sql/
├── Cargo.toml
├── Makefile                      # run/build/release/test/coverage/lint/fix/clean
├── LICENSE
├── README.md
├── docs/
│   ├── design.md                 # Architecture & data model documentation
│   ├── roadmap.md                # Version milestones & progress
│   └── tests/
│       ├── where-filter.md       # WHERE filter test plan
│       └── order-filter.md       # ORDER BY filter test plan
├── src/
│   ├── main.rs                   # tokio runtime + eframe::run_native
│   ├── app.rs                    # App struct, eframe::App impl, process_events()
│   ├── lib.rs                    # pub mod db, error, storage
│   ├── error.rs                  # AppError enum (10 variants) + Result<T> alias
│   │
│   ├── db/                       # Database abstraction layer
│   │   ├── mod.rs                # Module declarations (5 drivers commented out)
│   │   ├── connection.rs         # DriverType, DriverParams, SshConfig, TlsConfig, ConnectionConfig
│   │   ├── driver.rs             # DatabaseDriver trait + DbCommand (11) + DbEvent (8)
│   │   ├── factory.rs            # DbFactory::create() — only Postgres active
│   │   ├── types.rs              # DbValue, ColumnMeta, QueryResult, SchemaTree, all node types
│   │   ├── worker.rs             # DbWorker — async command/event loop
│   │   ├── postgres/             # Active driver
│   │   │   ├── mod.rs            # PostgresDriver with per-database pool cache
│   │   │   ├── connection_url.rs # Build connection URL from DriverParams
│   │   │   ├── type_mapping.rs   # Map Postgres types to DbValue
│   │   │   ├── schema_loader.rs  # Lazy schema loading (list_databases/schemas/detail)
│   │   │   └── queries.rs        # READ ONLY transactions, COUNT(*) pagination
│   │   ├── sqlite.rs             # Written, tested, commented out
│   │   ├── mysql.rs              # Written, tested, commented out
│   │   ├── redis_driver.rs       # Written, tested, commented out
│   │   ├── mongodb_driver.rs     # Written, tested, commented out
│   │   └── mssql.rs              # Written, tested, commented out
│   │
│   ├── ui/                       # UI layer (egui) — used directly by app.rs
│   │   ├── mod.rs                # Re-exports all UI components
│   │   ├── sidebar.rs            # Schema tree with lazy loading, database picker
│   │   ├── tab_manager.rs        # TabManager, TabEntry, TabKind, tab bar
│   │   ├── sql_editor_tab.rs     # SQL editor with CodeEditor widget + Run button
│   │   ├── table_viewer_tab.rs   # Table viewer, filter bar, cell editor, pagination
│   │   ├── result_grid.rs        # Display cache, fixed_cell helper, virtual scrolling
│   │   ├── connection_dialog.rs  # Modal dialog for all 6 DB types, edit mode
│   │   └── statusbar.rs          # Status bar
│   │
│   └── storage/                  # Persistence
│       ├── mod.rs
│       └── config.rs             # AppConfig — save/load TOML connections
│
└── tests/                        # Integration tests (testcontainers)
    ├── postgres_driver_test.rs   # 14 tests
    ├── sqlite_driver_test.rs     # 12 tests (:memory:)
    ├── mysql_driver_test.rs      # 11 tests
    ├── redis_driver_test.rs      # 10 tests
    ├── mongodb_driver_test.rs    # 10 tests
    └── mssql_driver_test.rs      # 10 tests (Apple Silicon incompatible)
```

## Architecture

DB queries run on a tokio background worker and communicate with the UI thread via `mpsc` channels, keeping the UI completely non-blocking.

```
UI thread (eframe + egui)
    │  send DbCommand via mpsc::Sender
    ▼
DbWorker (tokio::spawn)
    │  owns all Box<dyn DatabaseDriver>
    │  processes commands, runs queries
    │  send DbEvent via mpsc::Sender
    ▼
UI thread  ←  poll events each frame, update state & request repaint
```

### Key Design Decisions

- **Lazy 3-level schema loading**: Connect -> `list_databases()` -> click database -> `list_schemas(db)` -> click schema -> `load_schema_detail(db, schema)` loads tables/views/columns/indexes/FKs
- **Per-database connection pools**: PostgresDriver caches `PgPool` per database since Postgres doesn't support cross-database queries
- **READ ONLY transactions**: `table_data()` wraps user input (WHERE/ORDER BY) in `BEGIN; SET TRANSACTION READ ONLY; ... COMMIT;` to prevent SQL injection
- **Virtual scrolling**: `ScrollArea::show_rows()` — only visible rows rendered in data grid
- **Display cache**: `Vec<Vec<String>>` built once on query result, zero per-frame allocation
- **Reactive repaint**: Only repaints on DB events or user input; polls at 33ms only when loading

## Development

```bash
# Run in development mode
make run

# Run unit tests only
make test

# Run all tests (requires Docker for testcontainers)
make test-all

# Code coverage (requires cargo-tarpaulin)
make coverage

# Lint
make lint

# Build release binary
make release
```

## License

[Apache 2.0](./LICENSE)
