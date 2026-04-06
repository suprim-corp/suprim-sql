use std::collections::HashMap;

use sqlx::{AssertSqlSafe, Row};
use sqlx::postgres::PgPool;

use crate::db::types::{
    ColumnNode, DatabaseNode, ForeignKeyNode, IndexNode, SchemaNode, SchemaTree, TableNode,
    ViewNode,
};
use crate::error::{AppError, Result};

/// Load the schema tree (lazy): returns ALL databases with schema names only (no tables).
/// For each database we can only introspect schemas of the current connection's database;
/// other databases are listed but shown as expandable with no schemas pre-loaded.
pub async fn load_schema(pool: &PgPool) -> Result<SchemaTree> {
    // Get current database name (the one we're connected to).
    let current_db_row = sqlx::query("SELECT current_database() AS db")
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::Schema(e.to_string()))?;
    let current_db: String = current_db_row
        .try_get("db")
        .unwrap_or_else(|_| "postgres".to_string());

    // List all accessible databases from the server.
    let db_rows = sqlx::query(
        "SELECT datname FROM pg_catalog.pg_database \
         WHERE datistemplate = false \
         AND datallowconn = true \
         ORDER BY datname",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Schema(e.to_string()))?;

    // List schemas of the current database only (no tables/columns — loaded lazily).
    let schema_rows = sqlx::query(
        "SELECT schema_name FROM information_schema.schemata \
         WHERE schema_name NOT IN ('pg_catalog','information_schema','pg_toast') \
         AND schema_name NOT LIKE 'pg_toast_%' \
         AND schema_name NOT LIKE 'pg_temp_%' \
         ORDER BY schema_name",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Schema(e.to_string()))?;

    let current_schemas: Vec<SchemaNode> = schema_rows
        .iter()
        .map(|row| SchemaNode {
            id: uuid::Uuid::new_v4(),
            name: row.try_get("schema_name").unwrap_or_default(),
            tables: vec![],
            views: vec![],
            loaded: false,
        })
        .collect();

    // Build a DatabaseNode for each database.
    // Only the current database has schemas pre-populated; others show empty
    // (since cross-db queries aren't supported in PostgreSQL without dblink).
    let databases: Vec<DatabaseNode> = db_rows
        .iter()
        .map(|row| {
            let db_name: String = row.try_get("datname").unwrap_or_default();
            let schemas = if db_name == current_db {
                current_schemas.clone()
            } else {
                // Other databases: show one placeholder schema indicating
                // reconnection is needed to browse them.
                vec![]
            };
            DatabaseNode {
                id: uuid::Uuid::new_v4(),
                name: db_name,
                schemas,
            }
        })
        .collect();

    Ok(SchemaTree { databases })
}

/// Load full detail for a single named schema: tables, views, columns, indexes, FKs.
///
/// Uses batch queries to avoid N+1 — all columns, indexes, and FKs are fetched
/// for the entire schema in one query each, then grouped by table name in memory.
pub async fn load_schema_detail(pool: &PgPool, schema_name: &str) -> Result<SchemaNode> {
    // ── 1. Tables + views list ────────────────────────────────────────────────
    let table_rows = sqlx::query(AssertSqlSafe(
        "SELECT table_name, table_type \
         FROM information_schema.tables \
         WHERE table_schema = $1 \
         AND table_type IN ('BASE TABLE','VIEW') \
         ORDER BY table_name"
            .to_string(),
    ))
    .bind(schema_name)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Schema(e.to_string()))?;

    if table_rows.is_empty() {
        return Ok(SchemaNode {
            id: uuid::Uuid::new_v4(),
            name: schema_name.to_string(),
            tables: vec![],
            views: vec![],
            loaded: true,
        });
    }

    // ── 2. All columns for the schema (single batch query) ────────────────────
    let col_rows = sqlx::query(AssertSqlSafe(
        "SELECT table_name, column_name, udt_name, \
              (is_nullable = 'YES') AS is_nullable, \
              column_default, ordinal_position
         FROM information_schema.columns
         WHERE table_schema = $1
         ORDER BY table_name, ordinal_position"
            .to_string(),
    ))
    .bind(schema_name)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Schema(e.to_string()))?;

    // ── 3. Primary key columns for the schema (single batch query) ────────────
    let pk_sql = format!(
        "SELECT t.relname AS table_name, a.attname AS col_name \
         FROM pg_catalog.pg_constraint c \
         JOIN pg_catalog.pg_class t ON t.oid = c.conrelid \
         JOIN pg_catalog.pg_namespace n ON n.oid = t.relnamespace \
         JOIN pg_catalog.pg_attribute a ON a.attrelid = t.oid \
              AND a.attnum = ANY(c.conkey) \
         WHERE n.nspname = '{}' AND c.contype = 'p'",
        schema_name
    );
    let pk_rows = sqlx::query(AssertSqlSafe(pk_sql))
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    // pk_set: table_name → Set<col_name>
    let mut pk_set: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    for r in &pk_rows {
        let tbl: String = r.try_get("table_name").unwrap_or_default();
        let col: String = r.try_get("col_name").unwrap_or_default();
        pk_set.entry(tbl).or_default().insert(col);
    }

    // ── 4. Indexes for the schema (single batch query) ────────────────────────
    let idx_sql = format!(
        "SELECT t.relname AS table_name, \
                i.relname AS index_name, \
                ix.indisunique AS is_unique, \
                array_to_string( \
                    ARRAY( \
                        SELECT a.attname \
                        FROM pg_catalog.pg_attribute a \
                        WHERE a.attrelid = t.oid \
                        AND a.attnum = ANY(ix.indkey) \
                        ORDER BY array_position(ix.indkey, a.attnum) \
                    ), ',' \
                ) AS column_names \
         FROM pg_catalog.pg_class t \
         JOIN pg_catalog.pg_index ix ON t.oid = ix.indrelid \
         JOIN pg_catalog.pg_class i ON i.oid = ix.indexrelid \
         JOIN pg_catalog.pg_namespace n ON n.oid = t.relnamespace \
         WHERE n.nspname = '{}' \
         ORDER BY t.relname, i.relname",
        schema_name
    );
    let idx_rows = sqlx::query(AssertSqlSafe(idx_sql))
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    // index_map: table_name → Vec<IndexNode>
    let mut index_map: HashMap<String, Vec<IndexNode>> = HashMap::new();
    for r in &idx_rows {
        let tbl: String = r.try_get("table_name").unwrap_or_default();
        index_map.entry(tbl).or_default().push(IndexNode {
            id: uuid::Uuid::new_v4(),
            name: r.try_get("index_name").unwrap_or_default(),
            columns: r
                .try_get::<String, _>("column_names")
                .unwrap_or_default()
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect(),
            is_unique: r.try_get("is_unique").unwrap_or(false),
        });
    }

    // ── 5. Foreign keys for the schema (single batch query) ───────────────────
    let fk_sql = format!(
        "SELECT \
             tc.table_name, \
             tc.constraint_name, \
             kcu.column_name, \
             ccu.table_name AS ref_table, \
             ccu.column_name AS ref_column \
         FROM information_schema.table_constraints tc \
         JOIN information_schema.key_column_usage kcu \
              ON tc.constraint_name = kcu.constraint_name \
              AND tc.table_schema = kcu.table_schema \
         JOIN information_schema.constraint_column_usage ccu \
              ON tc.constraint_name = ccu.constraint_name \
         WHERE tc.constraint_type = 'FOREIGN KEY' \
           AND tc.table_schema = '{}' \
         ORDER BY tc.table_name, tc.constraint_name, kcu.ordinal_position",
        schema_name
    );
    let fk_rows = sqlx::query(AssertSqlSafe(fk_sql))
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    // fk_map: table_name → HashMap<constraint_name → ForeignKeyNode>
    let mut fk_outer: HashMap<String, HashMap<String, ForeignKeyNode>> = HashMap::new();
    for r in &fk_rows {
        let tbl: String = r.try_get("table_name").unwrap_or_default();
        let constraint: String = r.try_get("constraint_name").unwrap_or_default();
        let col: String = r.try_get("column_name").unwrap_or_default();
        let ref_table: String = r.try_get("ref_table").unwrap_or_default();
        let ref_col: String = r.try_get("ref_column").unwrap_or_default();
        let inner = fk_outer.entry(tbl).or_default();
        let fk = inner.entry(constraint.clone()).or_insert(ForeignKeyNode {
            id: uuid::Uuid::new_v4(),
            name: constraint,
            columns: Vec::new(),
            ref_table,
            ref_columns: Vec::new(),
        });
        fk.columns.push(col);
        fk.ref_columns.push(ref_col);
    }

    // ── 6. Build columns map: table_name → Vec<ColumnNode> ────────────────────
    let mut col_map: HashMap<String, Vec<ColumnNode>> = HashMap::new();
    for r in &col_rows {
        let tbl: String = r.try_get("table_name").unwrap_or_default();
        let col_name: String = r.try_get("column_name").unwrap_or_default();
        let is_pk = pk_set
            .get(&tbl)
            .map_or(false, |s| s.contains(&col_name));
        col_map.entry(tbl).or_default().push(ColumnNode {
            id: uuid::Uuid::new_v4(),
            name: col_name,
            db_type: r.try_get::<String, _>("udt_name").unwrap_or_default(),
            nullable: r.try_get::<bool, _>("is_nullable").unwrap_or(true),
            is_primary_key: is_pk,
            default_value: r
                .try_get::<Option<String>, _>("column_default")
                .unwrap_or(None),
        });
    }

    // ── 7. Assemble SchemaNode ────────────────────────────────────────────────
    let mut tables = Vec::new();
    let mut views = Vec::new();

    for table_row in &table_rows {
        let table_name: String = table_row.try_get("table_name").unwrap_or_default();
        let table_type: String = table_row.try_get("table_type").unwrap_or_default();
        let columns = col_map.remove(&table_name).unwrap_or_default();

        if table_type == "VIEW" {
            views.push(ViewNode {
                id: uuid::Uuid::new_v4(),
                name: table_name,
                columns,
            });
        } else {
            let indexes = index_map.remove(&table_name).unwrap_or_default();
            let foreign_keys = fk_outer
                .remove(&table_name)
                .unwrap_or_default()
                .into_values()
                .collect();

            tables.push(TableNode {
                id: uuid::Uuid::new_v4(),
                name: table_name,
                columns,
                indexes,
                foreign_keys,
                row_count: None,
            });
        }
    }

    Ok(SchemaNode {
        id: uuid::Uuid::new_v4(),
        name: schema_name.to_string(),
        tables,
        views,
        loaded: true,
    })
}
