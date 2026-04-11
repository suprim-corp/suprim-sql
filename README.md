# suprim-sql

A simple, cross-platform SQL client written in Rust. No Electron. No JVM. No bloat.

## Current Status

**v0.1** — PostgreSQL fully working (100 source files, 19 sidebar actions, 19 DB commands, 13 DB events). 5 other
drivers (SQLite, MySQL, Redis, MongoDB, MSSQL) are written and tested but commented out for faster initial shipping.

## Features (Implemented)

### Database

- **PostgreSQL support**: Full driver with per-database connection pooling, lazy schema loading, query execution
- **Schema Browser**: Lazy 3-level sidebar — databases → schemas → tables/views/materialized views/sequences/functions
- **Connection Manager**: Save/restore connections to `~/.config/suprim-sql/connections.toml`, edit/delete connections
  with confirmation dialog
- **Database Filter**: Pick which databases to show per connection (`visible_databases`)
- **New Database / New Schema**: Right-click context menus to create databases and schemas directly
- **Extension Support**: Detect and diff PostgreSQL extensions (pgvector, pgcrypto, etc.)

### SQL Editor

- **Custom Syntax Highlighting**: Hand-rolled tokenizer with Gruvbox palette (9 token types: keyword, type, function,
  constant, string, number, comment, punctuation, default)
- **SQL Autocomplete**: Context-aware keyword/type/function/constant suggestions
- **Multi-tab**: SQL editor tabs + table viewer tabs + table editor tabs, open/close/switch
- **Run queries** with results in virtual-scrolled data grid

### Table Management

- **Table Viewer**: Browse data with pagination, WHERE/ORDER BY filter bar with syntax highlighting, total row count
- **Table Editor**: Edit existing table structure (columns, types, defaults, PK, nullable) with SQL preview
- **New Table**: Create tables with column editor — type dropdown (30 PostgreSQL types), length/precision field, default
  value autocomplete with type-aware suggestions + schema functions
- **DDL Operations**: Truncate, Drop, Rename tables via context menu
- **Cell Inspector**: Click cell to view/edit, JSON syntax highlighting for JSON columns
- **Inline Editing**: INSERT/UPDATE/DELETE rows with SQL preview

### Structure Sync

- **Schema Comparison**: Compare schemas across connections/databases with full diff engine
- **Object Types**: Tables, columns, indexes, foreign keys, views, materialized views, sequences, functions, extensions
- **DDL Preview**: Syntax-highlighted preview of generated DDL script before execution
- **Selective Sync**: Checkbox-based selection of which changes to include
- **Extension Diff**: Detect missing/extra/version-different extensions, generate CREATE/DROP/ALTER EXTENSION DDL

### UI

- **Virtual Scrolling**: Only visible rows rendered — handles large result sets
- **Theme-adaptive**: All colors derived from system theme via `ui.visuals()`, dark/light mode
- **Phosphor Icons**: 1531 icons via `egui-phosphor`
- **macOS Native Menu**: Custom menu bar via objc2/AppKit integration
- **macOS .app Bundle**: `cargo-bundle` pipeline with Info.plist patching, optional codesign + DMG
- **Cross-platform**: macOS (Metal), Linux (OpenGL/Wayland/X11), Windows (DirectX)

## Features (Planned)

- SQLite, MySQL, Redis, MongoDB, MSSQL (drivers written, not yet active)
- SSH tunneling (`russh`)
- TLS/SSL connections
- Encrypted credentials via OS keychain (`keyring-rs`)
- AI assistant for SQL writing (`async-openai`)
- Export/Import: CSV, JSON, Excel
- ERD diagram
- Query history
- Structure Sync: Execute step (apply DDL to target)

## Tech Stack

| Layer                       | Library                                                                                                             | Status            |
|-----------------------------|---------------------------------------------------------------------------------------------------------------------|-------------------|
| UI                          | [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) 0.34 + [egui](https://github.com/emilk/egui) 0.34 | Active            |
| Icons                       | [egui-phosphor](https://github.com/amPerl/egui-phosphor) 0.12.0                                                     | Active            |
| SQL Highlighting            | Custom tokenizer (`sql_highlighter.rs`, Gruvbox palette)                                                            | Active            |
| JSON Editor                 | [egui_code_editor](https://github.com/nickeisenberg/egui_code_editor) 0.2                                           | Active            |
| Plotting                    | [egui_plot](https://crates.io/crates/egui_plot) 0.34                                                                | Active            |
| Docking                     | [egui_dock](https://crates.io/crates/egui_dock) 0.18                                                                | Active            |
| Notifications               | [egui-notify](https://crates.io/crates/egui-notify) 0.21                                                            | Active            |
| Async runtime               | [tokio](https://github.com/tokio-rs/tokio)                                                                          | Active            |
| PostgreSQL / MySQL / SQLite | [sqlx](https://github.com/launchbadge/sqlx) 0.9.0-alpha.1                                                           | Postgres active   |
| MongoDB                     | [mongodb](https://github.com/mongodb/mongo-rust-driver) 3                                                           | Written, inactive |
| Redis                       | [redis-rs](https://github.com/redis-rs/redis-rs) 1                                                                  | Written, inactive |
| MSSQL                       | [tiberius](https://github.com/prisma/tiberius) 0.11                                                                 | Written, inactive |
| SSH Tunnel                  | [russh](https://github.com/warp-tech/russh) 0.60                                                                    | Not yet used      |
| File Dialogs                | [rfd](https://github.com/PolyMeilex/rfd) 0.17                                                                       | Not yet used      |
| Credentials                 | [keyring-rs](https://github.com/hwchen/keyring-rs) 4.0.0-rc.3                                                       | Not yet used      |
| AI                          | [async-openai](https://github.com/64bit/async-openai) 0.34                                                          | Not yet used      |
| Serialization               | [serde](https://serde.rs) + [toml](https://github.com/toml-rs/toml)                                                 | Active            |
| Error handling              | [thiserror](https://github.com/dtolnay/thiserror)                                                                   | Active            |
| macOS native                | [objc2](https://github.com/madsmtm/objc2) + objc2-app-kit + objc2-foundation                                        | Active            |

## Render Backend

- **macOS** — Metal
- **Linux** — OpenGL / Wayland / X11
- **Windows** — DirectX

## Project Structure

```
suprim-sql/
├── Cargo.toml
├── Makefile                          # run/build/release/bundle/dmg/test/coverage/lint/fix/clean
├── LICENSE
├── README.md
├── assets/
│   ├── icons/                        # icon.icns, icon.ico, icon.png, multi-resolution PNGs
│   └── macos/Info.plist.extra        # Retina, GPU, App Nap, dark mode plist keys
├── scripts/build/
│   └── macos.sh                      # .app bundle + optional codesign + DMG pipeline
├── docs/
│   ├── design.md                     # Architecture & data model documentation
│   ├── roadmap.md                    # Version milestones & progress
│   └── tests/                        # Test plans (WHERE/ORDER BY filters)
│
├── src/
│   ├── main.rs                       # tokio runtime + eframe::run_native
│   ├── app.rs                        # App struct, eframe::App impl
│   ├── app_ui.rs                     # UI rendering, dialog dispatch, sidebar context wiring
│   ├── event_handler.rs              # DbEvent → UI state updates (13 event handlers)
│   ├── sidebar_action_handler.rs     # SidebarAction → DbCommand dispatch (19 actions)
│   ├── lib.rs                        # pub mod db, error, storage
│   ├── error.rs                      # AppError enum + Result<T> alias
│   │
│   ├── db/                           # Database abstraction layer
│   │   ├── mod.rs                    # Module declarations + re-exports
│   │   ├── connection.rs             # DriverType, DriverParams, SshConfig, TlsConfig, ConnectionConfig
│   │   ├── driver.rs                 # DatabaseDriver trait + DbCommand (19) + DbEvent (13)
│   │   ├── factory.rs                # DbFactory::create() — only Postgres active
│   │   ├── schema.rs                 # SchemaTree, SchemaNode, TableNode, ViewNode, FunctionNode, ExtensionInfo
│   │   ├── types.rs                  # DbValue, ColumnMeta, QueryResult
│   │   ├── values.rs                 # Value conversion helpers
│   │   │
│   │   ├── sql_keywords/             # SQL language definitions (shared by highlighter + autocomplete)
│   │   │   ├── keywords.rs           # ~100 SQL keywords
│   │   │   ├── types.rs              # ~55 PostgreSQL types
│   │   │   ├── functions.rs          # ~60 SQL functions
│   │   │   └── constants.rs          # TRUE, FALSE, NULL, CURRENT_TIMESTAMP, etc.
│   │   │
│   │   ├── drivers/                  # Database driver implementations
│   │   │   ├── postgres/             # Active driver (7 files)
│   │   │   │   ├── mod.rs            # PostgresDriver struct + per-database pool cache + PG_COLUMN_TYPES
│   │   │   │   ├── driver_impl.rs    # DatabaseDriver trait impl
│   │   │   │   ├── connection_url.rs # Build connection URL from DriverParams
│   │   │   │   ├── schema_loader.rs  # Lazy schema loading (databases/schemas/detail)
│   │   │   │   ├── function_loader.rs# Load functions from pg_proc (filters C functions)
│   │   │   │   ├── extension_loader.rs# Load extensions from pg_extension
│   │   │   │   ├── type_mapping.rs   # Map Postgres types to DbValue
│   │   │   │   ├── queries.rs        # READ ONLY transactions, COUNT(*) pagination
│   │   │   │   └── driver_tests.rs   # Unit tests
│   │   │   ├── sqlite/               # Written, inactive
│   │   │   ├── mysql/                # Written, inactive
│   │   │   ├── redis/                # Written, inactive
│   │   │   ├── mongodb/              # Written, inactive
│   │   │   └── mssql/                # Written, inactive
│   │   │
│   │   └── worker/                   # Async DB worker (background thread)
│   │       ├── mod.rs                # DbWorker — command/event loop + handle_ddl helper
│   │       ├── command_handlers.rs   # DbCommand dispatch (19 variants)
│   │       ├── handle_connection.rs  # Connect/disconnect/test handlers
│   │       ├── handle_query.rs       # Execute, list, load schema, compare schemas, create DB/schema
│   │       ├── handle_mutation.rs    # Insert/update/delete row handlers
│   │       └── tests.rs              # Worker unit tests
│   │
│   ├── ui/                           # UI layer (egui)
│   │   ├── mod.rs                    # Re-exports all UI components
│   │   ├── custom_title_bar.rs       # macOS custom title bar
│   │   ├── macos_menu.rs             # macOS native menu bar (objc2/AppKit)
│   │   ├── statusbar.rs              # Status bar
│   │   ├── tab_bar.rs                # Tab bar rendering
│   │   ├── tab_manager.rs            # TabManager — SQL/TableViewer/TableEditor tabs
│   │   │
│   │   ├── sidebar/                  # Schema browser sidebar (10 files)
│   │   │   ├── mod.rs                # Sidebar struct + connection management
│   │   │   ├── sidebar_action.rs     # SidebarAction enum (19 variants)
│   │   │   ├── sidebar_renderer.rs   # Connection headers + context menus
│   │   │   ├── schema_renderer.rs    # Database → schema → object tree rendering
│   │   │   ├── connection_entry.rs   # ConnectionEntry data model
│   │   │   ├── database_picker.rs    # Database visibility filter dialog
│   │   │   ├── tables_folder_renderer.rs    # Tables folder + New Table context menu
│   │   │   ├── views_folder_renderer.rs     # Views/Materialized Views folders
│   │   │   ├── sequences_folder_renderer.rs # Sequences folder
│   │   │   ├── table_detail_renderer.rs     # Columns/indexes/FKs under a table
│   │   │   ├── view_detail_renderer.rs      # View columns
│   │   │   └── table_context_menu.rs        # Table right-click: Edit, Truncate, Drop, Rename
│   │   │
│   │   ├── sql_editor/               # SQL editor module (4 files)
│   │   │   ├── sql_editor_tab.rs     # SQL editor tab with syntax highlighting + Run button
│   │   │   ├── sql_highlighter.rs    # Custom SQL tokenizer → egui LayoutJob (Gruvbox, 9 token types)
│   │   │   └── sql_autocomplete.rs   # Context-aware autocomplete popup
│   │   │
│   │   ├── table_viewer_tab/         # Table data viewer (6 files)
│   │   │   ├── mod.rs                # TableViewerTab main rendering
│   │   │   ├── filter_bar.rs         # WHERE/ORDER BY filter with syntax highlighting
│   │   │   ├── pagination_bar.rs     # Page navigation
│   │   │   ├── cell_editor.rs        # Cell edit modal
│   │   │   ├── cell_editor_widgets.rs# JSON code editor widget
│   │   │   └── cell_actions.rs       # Insert/update/delete row actions
│   │   │
│   │   ├── table_editor_tab/         # Table structure editor (5 files)
│   │   │   ├── mod.rs                # TableEditorTab — edit existing or create new table
│   │   │   ├── columns_grid.rs       # Editable columns grid (name, type, length, PK, nullable, default)
│   │   │   ├── default_suggestions.rs# Type-aware default value autocomplete
│   │   │   ├── detail_sections.rs    # Read-only indexes + foreign keys display
│   │   │   └── sql_generator.rs      # ALTER TABLE + CREATE TABLE SQL generation
│   │   │
│   │   ├── shared/                   # Shared UI utilities
│   │   │   ├── result_grid.rs        # Virtual-scrolled data grid with display cache
│   │   │   ├── result_grid_context_menu.rs  # Copy/export context menu
│   │   │   ├── clipboard_formatters.rs      # Copy as CSV/JSON/SQL formatters
│   │   │   └── editor_themes.rs             # Gruvbox code theme for JSON editor
│   │   │
│   │   └── dialog/                   # Modal dialogs
│   │       ├── about_dialog.rs       # About dialog
│   │       ├── connection_dialog.rs  # Create/edit connection dialog
│   │       ├── input_dialog.rs       # Reusable name input dialog (New Database, New Schema)
│   │       ├── delete_connection/    # Delete connection confirmation dialog
│   │       └── tool/
│   │           └── structure_sync/   # Structure Synchronization wizard (17 files)
│   │               ├── dialog.rs     # Main render loop + state machine
│   │               ├── state.rs      # Dialog state + on_schemas_compared()
│   │               ├── types.rs      # CompareState, DiffEntry, DiffGroup, ObjectType (9 variants)
│   │               ├── bottom_bar.rs # Navigation buttons
│   │               ├── diff_results_renderer.rs  # Diff tree with checkboxes
│   │               └── steps/
│   │                   ├── select/   # Endpoint pickers + info panels
│   │                   ├── compare/  # Diff engine + DDL generator (7 files)
│   │                   ├── preview/  # DDL preview with syntax highlighting
│   │                   ├── review/   # (stub)
│   │                   └── execute/  # (stub)
│   │
│   └── storage/                      # Persistence
│       └── config.rs                 # AppConfig — save/load TOML connections
│
└── tests/                            # Integration tests (testcontainers)
    ├── postgres_driver_test.rs
    ├── sqlite_driver_test.rs
    ├── mysql_driver_test.rs
    ├── redis_driver_test.rs
    ├── mongodb_driver_test.rs
    └── mssql_driver_test.rs
```

## Architecture

DB queries run on a tokio background worker and communicate with the UI thread via `mpsc` channels, keeping the UI
completely non-blocking.

```
UI thread (eframe + egui)
    │  send DbCommand (19 variants) via mpsc::Sender
    ▼
DbWorker (tokio::spawn)
    │  owns all Box<dyn DatabaseDriver>
    │  dispatches commands to handler modules
    │  send DbEvent (13 variants) via mpsc::Sender
    ▼
UI thread  ←  poll events each frame, update state & request repaint
```

### Key Design Decisions

- **Lazy 3-level schema loading**: Connect → `list_databases()` → click database → `list_schemas(db)` → click schema →
  `load_schema_detail(db, schema)` loads tables/views/columns/indexes/FKs/sequences/functions
- **Per-database connection pools**: PostgresDriver caches `PgPool` per database since Postgres doesn't support
  cross-database queries
- **READ ONLY transactions**: `table_data()` wraps user input (WHERE/ORDER BY) in
  `BEGIN; SET TRANSACTION READ ONLY; ... COMMIT;` to prevent SQL injection
- **Virtual scrolling**: `ScrollArea::show_rows()` — only visible rows rendered in data grid
- **Display cache**: `Vec<Vec<String>>` built once on query result, zero per-frame allocation
- **Reactive repaint**: Only repaints on DB events or user input; polls at 33ms only when loading
- **Custom SQL tokenizer**: Hand-rolled lexer in `sql_highlighter.rs` producing `egui::text::LayoutJob` with 9 token
  types and Gruvbox color palette — no external syntax highlighting crate
- **Modular sidebar actions**: 19 `SidebarAction` variants dispatched through `sidebar_action_handler.rs` — decouples UI
  events from DB commands

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

# Build macOS .app bundle (requires: cargo install cargo-bundle)
make bundle

# Build .app + code signing (set CODESIGN_IDENTITY env var)
make bundle-sign

# Build .app + .dmg installer (requires: brew install create-dmg)
make dmg

# Build .dmg + code signing (full pipeline)
make dmg-sign
```

## License

[Apache 2.0](./LICENSE)
