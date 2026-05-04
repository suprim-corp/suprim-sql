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

| Feature | Basic | Premium |
|---------|-------|---------|
| SQL Editor (syntax highlighting, autocomplete, formatter) | ✓ | ✓ |
| Data Grid (inline edit, add/delete rows, cell inspector) | ✓ | ✓ |
| Schema Browser (databases → schemas → tables/views/columns) | ✓ | ✓ |
| Export (CSV, JSON, SQL) | ✓ | ✓ |
| SSH Tunnel | ✓ | ✓ |
| TLS/SSL (5 modes, CA/client certs) | ✓ | ✓ |
| Server Dashboard (sessions, metrics, slow queries) | ✓ | ✓ |
| Query History (search, re-run, persistent) | ✓ | ✓ |
| Auto-update (macOS) | ✓ | ✓ |
| Structure Sync | — | ✓ |
| Export XLSX | — | ✓ |
| MongoDB driver | — | ✓ |
| SQL Server driver | — | ✓ |

## Supported Databases

| Database | Status | Tier |
|----------|--------|------|
| PostgreSQL | Supported | Free |
| MySQL | Supported | Free |
| SQLite | Coming soon | Free |
| Redis | Coming soon | Free |
| MongoDB | Supported | Premium |
| SQL Server | Supported | Premium |

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

### Build-time configuration

Two optional env vars are baked into the binary at compile time via
`option_env!`. Leave them unset for stock production builds.

| Variable                   | Default                                          | Purpose                                                                                   |
|----------------------------|--------------------------------------------------|-------------------------------------------------------------------------------------------|
| `SUPRIM_UPDATE_ENDPOINT`   | `https://api.suprim.dev/suprim/update/latest`    | Feed the self-updater polls. Override for staging / beta / local mock servers.            |
| `SUPRIM_TEAM_ID`           | unset → signature check skipped (`tracing::warn`) | Apple Developer ID Team ID (10 chars). Self-updater rejects DMGs signed by another identity. |

Three ways to set them:

```bash
# 1. Inline (one-off builds)
SUPRIM_UPDATE_ENDPOINT=https://staging.api.suprim.dev/suprim/update/latest \
SUPRIM_TEAM_ID=ABCDE12345 \
  cargo build --release

# 2. .env file (gitignored — copy the template first)
cp .env.example .env
# edit .env, then:
set -a && source .env && set +a
cargo build --release

# 3. .cargo/config.toml (gitignored — copy the template first)
cp .cargo/config.toml.example .cargo/config.toml
# edit, then every cargo build/run picks it up automatically
cargo build --release
```

> `option_env!` reads at *compile time*. Re-export the var and
> `cargo clean` if a new value seems stuck.

## Roadmap

- [ ] SQLite & Redis drivers
- [ ] AI assistant (natural language → SQL)
- [ ] ER diagram
- [ ] Import CSV
- [ ] Windows & Linux builds

## License

[AGPL-3.0](./LICENSE)
