# Roadmap — suprim-sql

## v0.1 — Core SQL Client (Done)
- [x] Kết nối PostgreSQL (per-database pool cache)
- [x] SQL editor với custom syntax highlighting (hand-rolled tokenizer, Gruvbox palette)
- [x] Chạy query, hiển thị kết quả dạng bảng (virtual scrolling, display cache)
- [x] Schema browser lazy 3-level (databases -> schemas -> tables/views/columns/indexes/FKs/sequences/functions)
- [x] Lưu và khôi phục connections (TOML: ~/.config/suprim-sql/connections.toml)
- [x] Multi-tab (SQL editor + table viewer + table editor + server dashboard)
- [x] Connection dialog hỗ trợ cấu hình tất cả 6 loại DB
- [x] Phosphor icons xuyên suốt UI
- [x] Theme-adaptive colors (derived from ui.visuals())

## v0.2 — Table Data Browsing (Done)
- [x] Browse table data với phân trang (page X / Y, N rows)
- [x] WHERE / ORDER BY filter bar
- [x] Total row count (COUNT(*) trong cùng READ ONLY transaction)
- [x] Cell inspector — click vào ô để xem chi tiết
- [x] JSON syntax highlighting cho JSON columns (egui_code_editor)
- [x] Copy cell value (Cmd+C)
- [x] Database filter — chọn databases hiển thị per connection
- [x] Sửa dữ liệu inline — batch mode với SQL preview trước khi commit
- [x] Thêm row (New Row editor popup với type-aware defaults)
- [x] Xóa row (context menu + toolbar, batch pending)
- [x] Undo last edit
- [x] Row selection (click row number)

## v0.3 — Multi-driver Activation (Partial)
- [ ] Bật SQLite driver (code done 944 LOC, commented out in drivers/mod.rs, not wired in factory)
- [x] Bật MySQL driver (active in drivers/mod.rs, wired in factory.rs, 7 source files)
- [ ] Bật Redis driver (code done 846 LOC, commented out in drivers/mod.rs, not wired in factory)
- [x] Bật MongoDB driver (active via premium gate in extensions crate, 878 LOC)
- [x] Bật MSSQL driver (active via premium gate in extensions crate, 684 LOC)

## v0.4 — Query Productivity (Done)
- [x] Query history — xem lại và chạy lại query cũ (Cmd+Y, bottom panel, search, persistent JSON)
- [x] Autocomplete (SQL keywords + types + functions + constants, column names from schema)
- [x] SQL formatter / prettify (sqlformat crate, Shift+Cmd+F)

## v0.5 — Security & Connectivity (Done)
- [x] SSH tunnel (russh 0.60, PEM/OpenSSH/PKCS8 key formats, RSA-SHA256)
- [x] TLS / SSL connections (SslMode 5 levels, CA/client cert file pickers, wired in PG + MySQL drivers)
- [x] Credentials encrypted at rest (AES-256-GCM with machine-derived key, auto-migration on save)
- [ ] OS keychain integration (keyring-rs dep exists, not called — stub only in extensions)

## v0.6 — Export & Import (Partial)
- [x] Export CSV (configurable delimiter, quoting, line breaks, gzip, formula sanitization)
- [x] Export JSON (pretty print, null inclusion, all-as-strings, gzip)
- [x] Export SQL (INSERT batching, DROP/CREATE TABLE DDL, dialect-aware quoting, gzip)
- [ ] Export XLSX (premium-gated, UI shows "Coming soon")
- [ ] Import CSV vào table (no implementation)
- [x] Export dialog — multi-table tree selection, format options, native save dialog
- [x] Clipboard copy — cell → JSON/CSV/SQL format

## v0.7 — AI Assistant
- [ ] Chat panel hỏi AI về SQL (async-openai)
- [ ] AI tự động viết query từ mô tả tự nhiên
- [ ] Giải thích query

## v0.8 — Schema Visualization
- [ ] ERD diagram — visualize schema dạng đồ thị
- [ ] Xem indexes, foreign keys, constraints trực quan

## v0.9 — Polish (Done)
- [x] Workspaces — lưu layout, tab state, auto-reconnect (workspace.json)
- [x] Auto-update (check api.suprim.dev, download DMG, SHA-256 verify, atomic install + rollback, codesign Team ID verify, relaunch — macOS only)

## v1.0 — Release (Partial)
- [x] macOS `.app` bundle + `.dmg` (cargo-bundle + create-dmg, codesign support)
- [ ] Linux `AppImage` + `.deb`
- [ ] Windows `.msi`
- [x] Freemium model (PremiumGate trait, PremiumLicense, Free: PG/SQLite/MySQL/Redis + 5 connections, Premium: +MongoDB/MSSQL + unlimited, upgrade prompt dialog)

---

### Bonus Features (ngoài roadmap)

- [x] Structure Sync — diff 2 schemas, generate DDL, 5-step wizard, premium-gated (extensions)
- [x] Server Dashboard — active sessions, 8 metrics, slow queries, auto-refresh, kill session (PG + MySQL)
- [x] New Table editor — column grid với type dropdown, default autocomplete, CREATE TABLE DDL
- [x] New Database / New Schema — input dialog + DDL execution
- [x] Delete Connection — confirmation dialog
- [x] Functions/Procedures support — pg_proc + MySQL INFORMATION_SCHEMA.ROUTINES loading
- [x] Extensions support — pg_extension loading, diff, DDL (CREATE/DROP/ALTER EXTENSION)
- [x] DDL Preview step — syntax-highlighted SQL viewer in Structure Sync
- [x] egui_kittest UI tests — 16 tests for dialogs and preview components
- [x] macOS native menu bar (objc2, NSMenu/NSMenuItem)

---

### Driver Status

| Driver | Code | Tests | Active | Tier |
|--------|------|-------|--------|------|
| PostgreSQL | Done | 14 tests | Yes | Free |
| MySQL | Done (7 files) | 11 tests | Yes | Free |
| SQLite | Done (944 LOC) | 12 tests | Commented out | Free (planned) |
| Redis | Done (846 LOC) | 10 tests | Commented out | Free (planned) |
| MongoDB | Done (878 LOC) | 10 tests | Yes (premium gate) | Premium |
| MSSQL | Done (684 LOC) | 10 tests | Yes (premium gate) | Premium |

### Test Summary

| Category | Count |
|----------|-------|
| Library unit tests | 74 |
| UI component tests (egui_kittest) | 16 |
| **Total** | **90** |

---

### Remaining Work (ưu tiên)

| # | Item | Effort | Notes |
|---|------|--------|-------|
| 1 | Bật SQLite + Redis drivers | ~2-3h | Code done, cần uncomment + wire factory + fix trait drift |
| 2 | MySQL feature parity | 3-4 ngày | WHERE/ORDER BY, COUNT, dashboard, backtick quoting (plan exists) |
| 3 | OS keychain integration | ~1 ngày | keyring-rs dep sẵn, cần wire vào credential storage |
| 4 | XLSX export (premium) | ~1 ngày | Stub exists, cần implement writer |
| 5 | Import CSV | ~1 ngày | No code yet |
| 6 | AI Assistant | 3-5 ngày | No code yet |
| 7 | ERD diagram | 2-3 ngày | No code yet |
| 8 | Linux build | 1-2 ngày | No scripts yet |
| 9 | Windows build | 1-2 ngày | No scripts yet |
