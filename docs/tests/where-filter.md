# Test Plan — WHERE Filter (table_data)

## Context
`table_data()` nhận `where_clause: Option<&str>` từ UI, concat vào SQL.
Chạy trong **READ ONLY transaction** để chặn mutation injection.

---

## Unit Tests — SQL Building (không cần DB)

| # | Test case | Input | Expected |
|---|-----------|-------|----------|
| 1 | Baseline — None | `where=None` | SQL không có WHERE |
| 2 | Valid WHERE | `where=Some("status = 'active'")` | `...WHERE status = 'active'\nLIMIT...` |
| 3 | Empty string → skip | `where=Some("")` | Không append WHERE |
| 4 | Whitespace-only → skip | `where=Some("   ")` | Trim rồi bỏ qua |

## Integration Tests — Security (cần real Postgres)

| # | Test case | WHERE input | Expected |
|---|-----------|-------------|----------|
| 5 | DELETE injection | `"1=1; DELETE FROM x"` | `Err(AppError::Query)` — read-only tx chặn |
| 6 | DROP TABLE injection | `"1=1; DROP TABLE x"` | `Err` — read-only tx chặn |
| 7 | INSERT injection | `"1=1); INSERT INTO x VALUES(1"` | `Err` — read-only tx chặn |
| 8 | UPDATE injection | `"1=1; UPDATE x SET col=1"` | `Err` — read-only tx chặn |

## Integration Tests — Functional (cần real Postgres)

| # | Test case | Input | Expected |
|---|-----------|-------|----------|
| 9 | Valid filter | `"id < 10"` | Chỉ trả rows có id < 10 |
| 10 | Subquery | `"id IN (SELECT id FROM t WHERE active)"` | Hoạt động bình thường |
| 11 | Pagination + filter | `"status='active'", page=0 vs page=1` | Offset đúng, rows khác nhau |
| 12 | Syntax error | `"invalid %%% sql"` | `Err(AppError::Query)` — không crash |
| 13 | Special chars | `"name = 'O''Brien'"` | Escape đúng hoặc lỗi syntax, không crash |
| 14 | Cross-database | `database=Some("other_db"), where=Some(...)` | Dùng đúng pool, filter hoạt động |

## Stress / Edge Tests

| # | Test case | Input | Expected |
|---|-----------|-------|----------|
| 15 | WHERE rất dài (>10KB) | `"col1=1 AND col2=2 AND ... (>10KB)"` | Không crash, trả lỗi hoặc kết quả |

---

**Total: 15 test cases** (4 unit + 4 security + 6 functional + 1 stress)
