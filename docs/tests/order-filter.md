# Test Plan — ORDER BY Filter (table_data)

## Context
`table_data()` nhận `order_clause: Option<&str>` từ UI, concat vào SQL.
Chạy trong **READ ONLY transaction** để chặn mutation injection.

---

## Unit Tests — SQL Building (không cần DB)

| # | Test case | Input | Expected |
|---|-----------|-------|----------|
| 1 | Baseline — None | `order=None` | SQL không có ORDER BY |
| 2 | Valid ORDER BY | `order=Some("id DESC")` | `...ORDER BY id DESC\nLIMIT...` |
| 3 | Empty string → skip | `order=Some("")` | Không append ORDER BY |
| 4 | Whitespace-only → skip | `order=Some("\t")` | Trim rồi bỏ qua |
| 5 | Combined WHERE + ORDER | `where=Some("age > 18"), order=Some("name")` | WHERE trước ORDER BY trước LIMIT |

## Integration Tests — Security (cần real Postgres)

| # | Test case | ORDER BY input | Expected |
|---|-----------|----------------|----------|
| 6 | DROP injection | `"id; DROP TABLE x"` | `Err` — read-only tx chặn |
| 7 | DELETE injection | `"id; DELETE FROM x"` | `Err` — read-only tx chặn |
| 8 | INSERT injection | `"id; INSERT INTO x VALUES(1)"` | `Err` — read-only tx chặn |

## Integration Tests — Functional (cần real Postgres)

| # | Test case | Input | Expected |
|---|-----------|-------|----------|
| 9 | Single column ASC | `"id ASC"` | Rows sorted ascending |
| 10 | Single column DESC | `"id DESC"` | Rows sorted descending |
| 11 | Multi-column | `"status, created_at DESC"` | Sort by status asc, then created_at desc |
| 12 | Expression | `"LOWER(name)"` | Sort by lowercased name |
| 13 | Syntax error | `",,invalid"` | `Err(AppError::Query)` — không crash |
| 14 | Pagination + order | `"id DESC", page=0 vs page=1` | Offset đúng, thứ tự consistent |

## Stress / Edge Tests

| # | Test case | Input | Expected |
|---|-----------|-------|----------|
| 15 | Concurrent calls | 10 concurrent table_data với order khác nhau | Mỗi call tx riêng, không race |

---

**Total: 15 test cases** (5 unit + 3 security + 6 functional + 1 stress)
