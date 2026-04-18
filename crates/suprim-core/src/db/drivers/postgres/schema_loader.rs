use std::collections::HashMap;

use sqlx::{AssertSqlSafe, Row};
use sqlx::postgres::PgPool;

use crate::db::types::{
    ColumnNode, ForeignKeyNode, IndexNode, SchemaNode, SequenceNode, TableNode, ViewNode,
};
use crate::error::{AppError, Result};

/// List all accessible (non-template, connectable) databases on this PostgreSQL server.
pub async fn list_databases(pool: &PgPool) -> Result<Vec<String>> {
    let rows = sqlx::query(
        "SELECT datname FROM pg_catalog.pg_database \
         WHERE datistemplate = false \
         AND datallowconn = true \
         ORDER BY datname",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Schema(e.to_string()))?;

    Ok(rows
        .iter()
        .map(|r| r.try_get("datname").unwrap_or_default())
        .collect())
}

/// List schemas in the currently connected database (PostgreSQL cannot cross-db query).
pub async fn list_schemas(pool: &PgPool) -> Result<Vec<String>> {
    let rows = sqlx::query(
        "SELECT schema_name FROM information_schema.schemata \
         WHERE schema_name NOT IN ('pg_catalog','information_schema','pg_toast') \
         AND schema_name NOT LIKE 'pg_toast_%' \
         AND schema_name NOT LIKE 'pg_temp_%' \
         ORDER BY schema_name",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Schema(e.to_string()))?;

    Ok(rows
        .iter()
        .map(|r| r.try_get("schema_name").unwrap_or_default())
        .collect())
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

    // ── 1b. Materialized views ────────────────────────────────────────────────
    let matview_rows = sqlx::query(
        "SELECT matviewname AS name \
         FROM pg_catalog.pg_matviews \
         WHERE schemaname = $1 \
         ORDER BY matviewname",
    )
    .bind(schema_name)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // ── 1c. Sequences ─────────────────────────────────────────────────────────
    let seq_rows = sqlx::query(
        "SELECT s.sequencename AS sequence_name, \
                s.data_type, \
                s.start_value, \
                s.increment_by AS increment, \
                s.min_value, \
                s.max_value, \
                s.last_value, \
                owner_tbl.relname AS owner_table, \
                a.attname AS owner_column \
         FROM pg_catalog.pg_sequences s \
         JOIN pg_catalog.pg_class seq_cls \
              ON seq_cls.relname = s.sequencename \
         JOIN pg_catalog.pg_namespace seq_ns \
              ON seq_ns.oid = seq_cls.relnamespace \
              AND seq_ns.nspname = s.schemaname \
         LEFT JOIN pg_catalog.pg_depend dep \
              ON dep.objid = seq_cls.oid \
              AND dep.deptype = 'a' \
              AND dep.classid = 'pg_class'::regclass \
         LEFT JOIN pg_catalog.pg_class owner_tbl \
              ON owner_tbl.oid = dep.refobjid \
         LEFT JOIN pg_catalog.pg_attribute a \
              ON a.attrelid = dep.refobjid \
              AND a.attnum = dep.refobjsubid \
         WHERE s.schemaname = $1 \
         ORDER BY s.sequencename",
    )
    .bind(schema_name)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let sequences: Vec<SequenceNode> = seq_rows
        .iter()
        .map(|r| {
            let owner_table: Option<String> = r.try_get("owner_table").unwrap_or(None);
            let owner_column: Option<String> = r.try_get("owner_column").unwrap_or(None);
            let owner = match (owner_table, owner_column) {
                (Some(t), Some(c)) => Some(format!("{}.{}", t, c)),
                _ => None,
            };
            SequenceNode {
                id: uuid::Uuid::new_v4(),
                name: r.try_get("sequence_name").unwrap_or_default(),
                data_type: r
                    .try_get::<Option<String>, _>("data_type")
                    .unwrap_or(None)
                    .unwrap_or_else(|| "bigint".into()),
                start_value: r.try_get::<i64, _>("start_value").unwrap_or(1),
                increment: r.try_get::<i64, _>("increment").unwrap_or(1),
                min_value: r.try_get::<i64, _>("min_value").unwrap_or(1),
                max_value: r.try_get::<i64, _>("max_value").unwrap_or(i64::MAX),
                last_value: r.try_get::<Option<i64>, _>("last_value").unwrap_or(None),
                owner,
            }
        })
        .collect();

    // ── 1d. Functions and procedures ──────────────────────────────────────────
    let functions = super::function_loader::load_functions(pool, schema_name).await;

    let matview_names: Vec<String> = matview_rows
        .iter()
        .map(|r| r.try_get::<String, _>("name").unwrap_or_default())
        .collect();

    if table_rows.is_empty() && matview_names.is_empty() {
        return Ok(SchemaNode {
            id: uuid::Uuid::new_v4(),
            name: schema_name.to_string(),
            tables: vec![],
            views: vec![],
            materialized_views: vec![],
            sequences,
            functions,
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
    let pk_rows = sqlx::query(
        "SELECT t.relname AS table_name, a.attname AS col_name \
         FROM pg_catalog.pg_constraint c \
         JOIN pg_catalog.pg_class t ON t.oid = c.conrelid \
         JOIN pg_catalog.pg_namespace n ON n.oid = t.relnamespace \
         JOIN pg_catalog.pg_attribute a ON a.attrelid = t.oid \
              AND a.attnum = ANY(c.conkey) \
         WHERE n.nspname = $1 AND c.contype = 'p'",
    )
    .bind(schema_name)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let mut pk_set: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
    for r in &pk_rows {
        let tbl: String = r.try_get("table_name").unwrap_or_default();
        let col: String = r.try_get("col_name").unwrap_or_default();
        pk_set.entry(tbl).or_default().insert(col);
    }

    // ── 4. Indexes for the schema (single batch query) ────────────────────────
    let idx_rows = sqlx::query(
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
         WHERE n.nspname = $1 \
         ORDER BY t.relname, i.relname",
    )
    .bind(schema_name)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
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
    let fk_rows = sqlx::query(
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
           AND tc.table_schema = $1 \
         ORDER BY tc.table_name, tc.constraint_name, kcu.ordinal_position",
    )
    .bind(schema_name)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
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
            .is_some_and(|s| s.contains(&col_name));
        col_map.entry(tbl).or_default().push(ColumnNode {
            id: uuid::Uuid::new_v4(),
            name: col_name,
            db_type: pg_type_display(&r.try_get::<String, _>("udt_name").unwrap_or_default()),
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

    // ── 7b. Materialized views ───────────────────────────────────────────────
    let mut materialized_views = Vec::new();
    for mv_name in &matview_names {
        let columns = col_map.remove(mv_name).unwrap_or_default();
        materialized_views.push(ViewNode {
            id: uuid::Uuid::new_v4(),
            name: mv_name.clone(),
            columns,
        });
    }

    Ok(SchemaNode {
        id: uuid::Uuid::new_v4(),
        name: schema_name.to_string(),
        tables,
        views,
        materialized_views,
        sequences,
        functions,
        loaded: true,
    })
}

/// Map PostgreSQL internal type names to SQL-standard / human-readable names.
fn pg_type_display(udt_name: &str) -> String {
    match udt_name {
        "int2" => "smallint".into(),
        "int4" => "integer".into(),
        "int8" => "bigint".into(),
        "float4" => "real".into(),
        "float8" => "double precision".into(),
        "bool" => "boolean".into(),
        "bpchar" => "char".into(),
        "timetz" => "time with time zone".into(),
        "timestamptz" => "timestamp with time zone".into(),
        "_int4" => "integer[]".into(),
        "_int8" => "bigint[]".into(),
        "_text" => "text[]".into(),
        "_varchar" => "varchar[]".into(),
        "_float4" => "real[]".into(),
        "_float8" => "double precision[]".into(),
        "_bool" => "boolean[]".into(),
        "_uuid" => "uuid[]".into(),
        other => other.into(),
    }
}
