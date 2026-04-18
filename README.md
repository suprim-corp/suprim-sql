<p align="center">
  <img src="assets/icons/icon.png" width="128" height="128" alt="SuprimSQL">
</p>

<h1 align="center">SuprimSQL</h1>

<p align="center">
  <strong>A fast, native database management tool.</strong><br>
  Built with Rust. No Electron. No JVM. No bloat.
</p>

<p align="center">
  <a href="https://github.com/suprim-corp/suprim-sql/releases/latest">Download</a> ·
  <a href="https://suprim.dev">Website</a> ·
  <a href="#features">Features</a>
</p>

---

## Why SuprimSQL?

Most database tools are either slow (Electron-based), expensive (per-seat licensing), or both.

SuprimSQL is a **native desktop app** — instant startup, low memory, smooth scrolling on large datasets. Written in Rust with [egui](https://github.com/emilk/egui), it renders at 60fps using your GPU (Metal on macOS, Vulkan/OpenGL on Linux, DirectX on Windows).

## Download

| Platform | Download | Architecture |
|----------|----------|-------------|
| **macOS** | [SuprimSQL.dmg](https://github.com/suprim-corp/suprim-sql/releases/latest) | Universal (Apple Silicon + Intel) |
| Windows | Coming soon | — |
| Linux | Coming soon | — |

> **Note:** macOS will show "unidentified developer" warning on first launch. Right-click the app → Open → Open to bypass.

## Features

### SQL Editor
Write and run SQL with syntax highlighting, autocomplete, and a built-in formatter. Multi-tab support — work on multiple queries side by side.

### Data Grid
Browse table data with virtual scrolling (handles millions of rows), inline cell editing, and batch operations. Click column headers to sort, use per-column filter popups to narrow down data.

### Schema Browser
Explore your database structure — tables, views, indexes, foreign keys, sequences, functions — in a lazy-loading sidebar tree.

### Export
Export to **CSV**, **JSON**, or **SQL** (with full DDL — CREATE TABLE + indexes + FKs). Multi-table export, gzip compression, and format-specific options.

### Server Dashboard
Monitor active sessions, transactions, cache hit ratio, and slow queries. Kill sessions directly from the UI.

### SSH Tunnel
Connect through SSH jump hosts with support for all key formats (Ed25519, RSA, ECDSA) and agent forwarding.

### TLS/SSL
Connect securely with configurable SSL modes — Disable, Prefer, Require, or Verify CA.

### Structure Sync *(Premium)*
Compare schemas across connections, review diffs, and generate DDL scripts to synchronize structure between databases.

## Supported Databases

| Database | Status |
|----------|--------|
| PostgreSQL | Fully supported |
| MySQL | Coming soon |
| SQLite | Coming soon |
| Redis | Coming soon |
| MongoDB | Coming soon (Premium) |
| SQL Server | Coming soon (Premium) |

## Build from Source

```bash
# Prerequisites: Rust 1.92+, macOS 12+
git clone https://github.com/suprim-corp/suprim-sql.git
cd suprim-sql

# Run in dev mode
make dev

# Build .app bundle
make bundle

# Build .dmg installer (requires: brew install create-dmg)
make dmg
```

## Roadmap

- [ ] Saved queries
- [ ] Connection folders
- [ ] AI assistant (natural language → SQL)
- [ ] ER diagram
- [ ] Auto-update
- [ ] Windows & Linux builds

## License

[AGPL-3.0](./LICENSE)
