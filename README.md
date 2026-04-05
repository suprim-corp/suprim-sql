# suprim-sql

A simple, cross-platform SQL client written in Rust. No Electron. No JVM. No bloat.

## Features

- **Multi-database support**: SQLite, PostgreSQL, MySQL, MariaDB, MongoDB, Redis, MSSQL
- **SQL Editor**: Syntax highlighting, autocomplete, query history
- **Table Viewer**: Browse and edit table data inline — changes generate proper SQL before committing
- **Schema Browser**: Sidebar for exploring tables, views, indexes, foreign keys
- **ERD Diagram**: Visual schema diagram tab
- **Connection Manager**: Saved connections with encrypted credentials (OS keychain)
- **SSH Tunneling + TLS**: Secure connections via tunnel
- **AI Assistant**: AI-powered SQL writing and explanation
- **Export / Import**: CSV, JSON, Excel
- **Cross-platform**: macOS, Linux, Windows — native binary, no runtime dependencies

## Tech Stack

| Layer | Library |
|---|---|
| UI | [egui](https://github.com/emilk/egui) + [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) |
| Async runtime | [tokio](https://github.com/tokio-rs/tokio) |
| PostgreSQL / MySQL / SQLite | [sqlx](https://github.com/launchbadge/sqlx) |
| MongoDB | [mongodb](https://github.com/mongodb/mongo-rust-driver) |
| Redis | [redis-rs](https://github.com/redis-rs/redis-rs) |
| MSSQL | [tiberius](https://github.com/prisma/tiberius) |
| SSH Tunnel | [russh](https://github.com/warp-tech/russh) |
| File Dialogs | [rfd](https://github.com/PolyMeilex/rfd) |
| Credentials | [keyring-rs](https://github.com/hwchen/keyring-rs) |
| AI | [async-openai](https://github.com/64bit/async-openai) |

## Render Backend

- **macOS** — Metal
- **Linux** — OpenGL / Wayland / X11
- **Windows** — DirectX

## Project Structure

```
suprim-sql/
├── Cargo.toml
├── build.rs                      # Build script (icon, resources)
├── assets/
│   ├── fonts/                    # Fonts embedded via include_bytes!
│   ├── icons/                    # App icon per platform
│   └── themes/                   # Color themes (TOML)
└── src/
    ├── main.rs                   # Entry point, tokio runtime + eframe
    ├── app.rs                    # AppState struct, eframe::App impl
    │
    ├── db/                       # Database layer
    │   ├── mod.rs                # DatabaseDriver trait, QueryResult, SchemaInfo
    │   ├── factory.rs            # DbFactory::create(ConnectionConfig)
    │   ├── connection.rs         # ConnectionConfig, ConnectionPool
    │   ├── sql_builder.rs        # Generate SELECT/INSERT/UPDATE/DELETE
    │   ├── schema.rs             # SchemaNode, TableInfo, ColumnInfo, IndexInfo
    │   ├── sqlite.rs
    │   ├── postgres.rs
    │   ├── mysql.rs
    │   ├── redis.rs
    │   ├── mongodb.rs
    │   └── mssql.rs
    │
    ├── ui/                       # UI layer (egui)
    │   ├── mod.rs
    │   ├── app_window.rs         # Main layout: sidebar + tabs + statusbar
    │   ├── sidebar.rs            # Schema tree (databases → tables → columns)
    │   ├── tab_manager.rs        # Open/close/switch tabs
    │   ├── toolbar.rs
    │   ├── statusbar.rs
    │   │
    │   ├── tab/                  # Tab types
    │   │   ├── mod.rs            # Tab trait
    │   │   ├── sql_editor_tab.rs # SQL editor + result grid
    │   │   ├── table_viewer_tab.rs   # Browse table data (paginated)
    │   │   ├── table_editor_tab.rs   # Inline edit → generate SQL
    │   │   ├── diagram_tab.rs    # ERD / schema diagram
    │   │   ├── redis_tab.rs      # Redis key browser + CLI
    │   │   ├── redis_pubsub_tab.rs
    │   │   └── mongo_tab.rs      # MongoDB collection browser
    │   │
    │   ├── editor/               # SQL text editor
    │   │   ├── mod.rs
    │   │   ├── highlight.rs      # Syntax highlighting
    │   │   ├── autocomplete.rs
    │   │   └── formatter.rs      # SQL format/prettify
    │   │
    │   ├── dialog/               # Modal dialogs
    │   │   ├── connection_dialog.rs
    │   │   ├── confirm_dialog.rs
    │   │   ├── ai_settings_dialog.rs
    │   │   └── update_dialog.rs
    │   │
    │   └── widgets/              # Custom egui widgets
    │       ├── table_renderer.rs # Scrollable data grid
    │       ├── spinner.rs
    │       └── toggle.rs
    │
    ├── ai/                       # AI assistant
    │   ├── mod.rs
    │   ├── chat_panel.rs
    │   └── provider.rs           # OpenAI / compatible API client
    │
    ├── tunnel/                   # SSH tunnel
    │   ├── mod.rs                # SshTunnel trait
    │   └── ssh.rs
    │
    ├── storage/                  # Persistence
    │   ├── mod.rs
    │   ├── config.rs             # Saved connections (TOML)
    │   ├── keychain.rs           # OS credential store (keyring-rs)
    │   ├── history.rs            # Query history (local SQLite)
    │   └── workspace.rs          # Workspaces, open tab state
    │
    ├── export/                   # Export / import
    │   ├── mod.rs
    │   ├── csv.rs
    │   ├── json.rs
    │   └── excel.rs
    │
    └── utils/
        ├── mod.rs
        ├── crypto.rs             # AES encrypt/decrypt credentials
        ├── format.rs             # Number/date formatting helpers
        └── updater.rs            # Check & download updates
```

## Architecture

DB queries run on a `tokio` background thread pool and communicate back to the UI thread via channels, keeping the UI non-blocking.

```
UI thread (egui)
    │  send DbCommand
    ▼
tokio runtime
    │  spawn query task
    │  receive QueryResult
    ▼
UI thread (egui)  ←  update state & repaint
```

## License

[Apache 2.0](./LICENSE)
