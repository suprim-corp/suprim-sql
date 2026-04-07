# Roadmap — suprim-sql

## v0.1 — Core SQL Client (Done)
- [x] Kết nối PostgreSQL (per-database pool cache)
- [x] SQL editor với syntax highlighting (egui_code_editor)
- [x] Chạy query, hiển thị kết quả dạng bảng (virtual scrolling, display cache)
- [x] Schema browser lazy 3-level (databases -> schemas -> tables/views/columns/indexes/FKs/sequences)
- [x] Lưu và khôi phục connections (TOML: ~/.config/suprim-sql/connections.toml)
- [x] Multi-tab (SQL editor + table viewer)
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
- [ ] Sửa dữ liệu inline — preview SQL trước khi commit
- [ ] Thêm / xóa row

## v0.3 — Multi-driver Activation
- [ ] Bật lại SQLite driver (đã viết + test, đang commented out)
- [ ] Bật lại MySQL driver (đã viết + test, đang commented out)
- [ ] Bật lại Redis driver (đã viết + test, đang commented out)
- [ ] Bật lại MongoDB driver (đã viết + test, đang commented out)
- [ ] Bật lại MSSQL driver (đã viết + test, đang commented out)

## v0.4 — Query Productivity
- [ ] Query history — xem lại và chạy lại query cũ
- [ ] Autocomplete (keywords + tên bảng/cột từ schema tree)
- [ ] SQL formatter / prettify

## v0.5 — Security & Connectivity
- [ ] SSH tunnel (russh 0.60)
- [ ] TLS / SSL connections
- [ ] Lưu credentials an toàn qua OS keychain (keyring-rs)

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

## v0.9 — Polish
- [ ] Workspaces — lưu layout và tab state
- [ ] Auto-update

## v1.0 — Release
- [ ] macOS `.dmg`, Linux `AppImage` + `.deb`, Windows `.msi`
- [ ] Freemium model: 3 connections miễn phí, Pro không giới hạn

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
