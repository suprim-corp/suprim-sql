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

## v0.3 — Multi-driver Activation
- [ ] Bật lại SQLite driver (đã viết + test, đang commented out)
- [ ] Bật lại MySQL driver (đã viết + test, đang commented out)
- [ ] Bật lại Redis driver (đã viết + test, đang commented out)
- [ ] Bật lại MongoDB driver (đã viết + test, đang commented out)
- [ ] Bật lại MSSQL driver (đã viết + test, đang commented out)

## v0.4 — Query Productivity (Done)
- [x] Query history — xem lại và chạy lại query cũ (Cmd+Y, bottom panel, search, persistent JSON)
- [x] Autocomplete (SQL keywords + types + functions + constants, column names from schema)
- [x] SQL formatter / prettify (sqlformat crate, Shift+Cmd+F)

## v0.5 — Security & Connectivity (Partial)
- [x] SSH tunnel (russh 0.60, PEM/OpenSSH/PKCS8 key formats, RSA-SHA256)
- [ ] TLS / SSL connections (data model exists, not wired)
- [ ] Lưu credentials an toàn qua OS keychain (keyring-rs data model exists, not wired)

## v0.6 — Export & Import
- [ ] Export kết quả query ra CSV, JSON, Excel
- [ ] Import CSV vào table

## v0.7 — AI Assistant
- [ ] Chat panel hỏi AI về SQL (async-openai)
- [ ] AI tự động viết query từ mô tả tự nhiên
- [ ] Giải thích query

## v0.8 — Schema Visualization
- [ ] ERD diagram — visualize schema dạng đồ thị
- [ ] Xem indexes, foreign keys, constraints trực quan

## v0.9 — Polish (Partial)
- [x] Workspaces — lưu layout, tab state, auto-reconnect (workspace.json)
- [ ] Auto-update

## v1.0 — Release (Partial)
- [x] macOS `.app` bundle + `.dmg` (cargo-bundle + create-dmg, codesign support)
- [ ] Linux `AppImage` + `.deb`
- [ ] Windows `.msi`
- [ ] Freemium model: 3 connections miễn phí, Pro không giới hạn

---

### Bonus Features (ngoài roadmap)

- [x] Structure Sync — diff 2 schemas, generate DDL, preview, extensions support
- [x] Server Dashboard — active sessions, 8 metrics, slow queries, auto-refresh, kill session
- [x] New Table editor — column grid với type dropdown, default autocomplete, CREATE TABLE DDL
- [x] New Database / New Schema — input dialog + DDL execution
- [x] Delete Connection — confirmation dialog
- [x] Functions/Procedures support — pg_proc loading, diff, DDL generation
- [x] Extensions support — pg_extension loading, diff, DDL (CREATE/DROP/ALTER EXTENSION)
- [x] DDL Preview step — syntax-highlighted SQL viewer in Structure Sync
- [x] egui_kittest UI tests — 16 tests for dialogs and preview components
- [x] macOS native menu bar (objc2)

---

### Driver Status

| Driver | Code | Tests | Active |
|--------|------|-------|--------|
| PostgreSQL | Done | 14 tests | Yes |
| SQLite | Done | 12 tests | Commented out |
| MySQL | Done | 11 tests | Commented out |
| Redis | Done | 10 tests | Commented out |
| MongoDB | Done | 10 tests | Commented out |
| MSSQL | Done | 10 tests | Commented out (Apple Silicon incompatible) |

### Test Summary

| Category | Count |
|----------|-------|
| Library unit tests | 74 |
| UI component tests (egui_kittest) | 16 |
| **Total** | **90** |
