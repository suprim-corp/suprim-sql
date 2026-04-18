//! Schema loading: list databases, list schemas, load schema detail (batch queries).
//!
//! MySQL databases ARE schemas — `list_schemas(db)` returns `[db]`.
//! `load_schema_detail(db)` loads all tables, views, columns, indexes, FKs,
//! and functions/procedures from INFORMATION_SCHEMA in batch (no N+1).

use std::collections::HashMap;

use sqlx::mysql::MySqlPool;
use sqlx::Row;

use crate::db::types::{
    ColumnNode, ForeignKeyNode, FunctionNode, IndexNode, SchemaNode, TableNode, ViewNode,
};
use crate::error::{AppError, Result};

/// List all databases on this MySQL server.
pub(super) async fn list_databases(pool: &MySqlPool) -> Result<Vec<String>> {
    let rows = sqlx::query("SHOW DATABASES")
        .fetch_all(pool)
        .await
        .map_err(|e| AppError::Schema(e.to_string()))?;

    Ok(rows
        .iter()
        .filter_map(|r| r.try_get::<String, _>(0).ok())
        .collect())
}

/// MySQL databases ARE schemas — return the database name itself.
pub(super) async fn list_schemas(database: &str) -> Result<Vec<String>> {
    Ok(vec![database.to_string()])
}

/// Load full detail for a MySQL database (= schema): tables, views, columns, indexes, FKs, functions.
///
/// Uses batch INFORMATION_SCHEMA queries to avoid N+1 per-table queries.
pub(super) async fn load_schema_detail(
    pool: &MySqlPool,
    database: &str,
) -> Result<SchemaNode> {
    // ── 1. Tables + views list ────────────────────────────────────────────────
    let table_rows = sqlx::query(
        "SELECT TABLE_NAME, TABLE_TYPE \
         FROM INFORMATION_SCHEMA.TABLES \
         WHERE TABLE_SCHEMA = ? \
         ORDER BY TABLE_TYPE, TABLE_NAME",
    )
    .bind(database)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Schema(e.to_string()))?;

    // ── 2. ALL columns for this database (single batch query) ─────────────────
    let col_rows = sqlx::query(
        "SELECT TABLE_NAME, COLUMN_NAME, COLUMN_TYPE, IS_NULLABLE, \
                COLUMN_DEFAULT, COLUMN_KEY, ORDINAL_POSITION \
         FROM INFORMATION_SCHEMA.COLUMNS \
         WHERE TABLE_SCHEMA = ? \
         ORDER BY TABLE_NAME, ORDINAL_POSITION",
    )
    .bind(database)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::Schema(e.to_string()))?;

    // Group columns by table name
    let mut col_map: HashMap<String, Vec<ColumnNode>> = HashMap::new();
    for r in &col_rows {
        let tbl: String = r.try_get("TABLE_NAME").unwrap_or_default();
        let col_name: String = r.try_get("COLUMN_NAME").unwrap_or_default();
        let col_type: String = r.try_get("COLUMN_TYPE").unwrap_or_default();
        let nullable: String = r.try_get("IS_NULLABLE").unwrap_or_else(|_| "YES".to_string());
        let key: String = r.try_get("COLUMN_KEY").unwrap_or_default();
        let dflt: Option<String> = r.try_get("COLUMN_DEFAULT").unwrap_or(None);

        col_map.entry(tbl).or_default().push(ColumnNode {
            id: uuid::Uuid::new_v4(),
            name: col_name,
            db_type: col_type,
            nullable: nullable == "YES",
            is_primary_key: key == "PRI",
            default_value: dflt,
        });
    }

    // ── 3. ALL indexes for this database (single batch query) ─────────────────
    let idx_rows = sqlx::query(
        "SELECT TABLE_NAME, INDEX_NAME, NON_UNIQUE, \
                GROUP_CONCAT(COLUMN_NAME ORDER BY SEQ_IN_INDEX) AS col_names \
         FROM INFORMATION_SCHEMA.STATISTICS \
         WHERE TABLE_SCHEMA = ? \
         GROUP BY TABLE_NAME, INDEX_NAME, NON_UNIQUE \
         ORDER BY TABLE_NAME, INDEX_NAME",
    )
    .bind(database)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut index_map: HashMap<String, Vec<IndexNode>> = HashMap::new();
    for r in &idx_rows {
        let tbl: String = r.try_get("TABLE_NAME").unwrap_or_default();
        let iname: String = r.try_get("INDEX_NAME").unwrap_or_default();
        let non_unique: i64 = r.try_get::<i64, _>("NON_UNIQUE").unwrap_or(1);
        let cols: String = r.try_get("col_names").unwrap_or_default();

        index_map.entry(tbl).or_default().push(IndexNode {
            id: uuid::Uuid::new_v4(),
            name: iname,
            columns: cols
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect(),
            is_unique: non_unique == 0,
        });
    }

    // ── 4. ALL foreign keys for this database (single batch query) ────────────
    let fk_rows = sqlx::query(
        "SELECT kcu.TABLE_NAME, kcu.CONSTRAINT_NAME, kcu.COLUMN_NAME, \
                kcu.REFERENCED_TABLE_NAME, kcu.REFERENCED_COLUMN_NAME \
         FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE kcu \
         JOIN INFORMATION_SCHEMA.TABLE_CONSTRAINTS tc \
              ON kcu.CONSTRAINT_NAME = tc.CONSTRAINT_NAME \
             AND kcu.TABLE_SCHEMA = tc.TABLE_SCHEMA \
             AND kcu.TABLE_NAME = tc.TABLE_NAME \
         WHERE kcu.TABLE_SCHEMA = ? \
           AND tc.CONSTRAINT_TYPE = 'FOREIGN KEY' \
         ORDER BY kcu.TABLE_NAME, kcu.CONSTRAINT_NAME, kcu.ORDINAL_POSITION",
    )
    .bind(database)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Group FK rows: table_name -> constraint_name -> ForeignKeyNode
    let mut fk_outer: HashMap<String, HashMap<String, ForeignKeyNode>> = HashMap::new();
    for r in &fk_rows {
        let tbl: String = r.try_get("TABLE_NAME").unwrap_or_default();
        let constraint: String = r.try_get("CONSTRAINT_NAME").unwrap_or_default();
        let col: String = r.try_get("COLUMN_NAME").unwrap_or_default();
        let ref_table: String = r.try_get("REFERENCED_TABLE_NAME").unwrap_or_default();
        let ref_col: String = r.try_get("REFERENCED_COLUMN_NAME").unwrap_or_default();

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

    // ── 5. Functions and procedures from INFORMATION_SCHEMA.ROUTINES ──────────
    let functions = load_functions(pool, database).await;

    // ── 6. Assemble SchemaNode ────────────────────────────────────────────────
    if table_rows.is_empty() {
        return Ok(SchemaNode {
            id: uuid::Uuid::new_v4(),
            name: database.to_string(),
            tables: vec![],
            views: vec![],
            materialized_views: vec![],
            sequences: vec![],
            functions,
            loaded: true,
        });
    }

    let mut tables = Vec::new();
    let mut views = Vec::new();

    for row in &table_rows {
        let tname: String = row.try_get("TABLE_NAME").unwrap_or_default();
        let ttype: String = row.try_get("TABLE_TYPE").unwrap_or_default();
        let columns = col_map.remove(&tname).unwrap_or_default();

        if ttype == "VIEW" {
            views.push(ViewNode {
                id: uuid::Uuid::new_v4(),
                name: tname,
                columns,
            });
        } else {
            let indexes = index_map.remove(&tname).unwrap_or_default();
            let foreign_keys = fk_outer
                .remove(&tname)
                .unwrap_or_default()
                .into_values()
                .collect();

            tables.push(TableNode {
                id: uuid::Uuid::new_v4(),
                name: tname,
                columns,
                indexes,
                foreign_keys,
                row_count: None,
            });
        }
    }

    Ok(SchemaNode {
        id: uuid::Uuid::new_v4(),
        name: database.to_string(),
        tables,
        views,
        materialized_views: vec![],
        sequences: vec![],
        functions,
        loaded: true,
    })
}

/// Load user-defined functions and procedures from INFORMATION_SCHEMA.ROUTINES.
async fn load_functions(pool: &MySqlPool, database: &str) -> Vec<FunctionNode> {
    let func_rows = sqlx::query(
        "SELECT ROUTINE_NAME, ROUTINE_TYPE, DTD_IDENTIFIER, \
                ROUTINE_DEFINITION, ROUTINE_BODY \
         FROM INFORMATION_SCHEMA.ROUTINES \
         WHERE ROUTINE_SCHEMA = ? \
         ORDER BY ROUTINE_TYPE, ROUTINE_NAME",
    )
    .bind(database)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Load parameters for each routine to build signatures
    let param_rows = sqlx::query(
        "SELECT SPECIFIC_NAME, PARAMETER_NAME, DATA_TYPE, PARAMETER_MODE, \
                ORDINAL_POSITION \
         FROM INFORMATION_SCHEMA.PARAMETERS \
         WHERE SPECIFIC_SCHEMA = ? \
           AND ORDINAL_POSITION > 0 \
         ORDER BY SPECIFIC_NAME, ORDINAL_POSITION",
    )
    .bind(database)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    // Group parameters by routine name
    let mut params_map: HashMap<String, Vec<String>> = HashMap::new();
    for r in &param_rows {
        let specific: String = r.try_get("SPECIFIC_NAME").unwrap_or_default();
        let param_name: String = r.try_get("PARAMETER_NAME").unwrap_or_default();
        let data_type: String = r.try_get("DATA_TYPE").unwrap_or_default();
        let mode: String = r.try_get("PARAMETER_MODE").unwrap_or_default();

        let display = if param_name.is_empty() {
            data_type
        } else {
            format!("{} {} {}", mode, param_name, data_type)
        };
        params_map.entry(specific).or_default().push(display);
    }

    func_rows
        .iter()
        .map(|r| {
            let name: String = r.try_get("ROUTINE_NAME").unwrap_or_default();
            let routine_type: String = r.try_get("ROUTINE_TYPE").unwrap_or_default();
            let return_type: String = r.try_get("DTD_IDENTIFIER").unwrap_or_default();
            let definition: String = r.try_get("ROUTINE_DEFINITION").unwrap_or_default();
            let is_procedure = routine_type == "PROCEDURE";

            let identity_args = params_map
                .get(&name)
                .map(|p| p.join(", "))
                .unwrap_or_default();

            let signature = if identity_args.is_empty() {
                format!("{}()", name)
            } else {
                format!("{}({})", name, identity_args)
            };

            FunctionNode {
                id: uuid::Uuid::new_v4(),
                name,
                identity_args,
                signature,
                return_type: if is_procedure {
                    String::new()
                } else {
                    return_type
                },
                language: "SQL".to_string(),
                definition,
                is_procedure,
            }
        })
        .collect()
}
