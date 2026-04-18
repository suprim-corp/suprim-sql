//! Load functions and procedures from PostgreSQL catalog.

use sqlx::postgres::PgPool;
use sqlx::Row;

use crate::db::types::FunctionNode;

/// Load user-defined functions and procedures for a given schema from `pg_catalog.pg_proc`.
/// Excludes C-language functions (from extensions) — those are handled at extension level.
pub(super) async fn load_functions(pool: &PgPool, schema_name: &str) -> Vec<FunctionNode> {
    let func_rows = sqlx::query(
        "SELECT p.proname AS func_name, \
                pg_catalog.pg_get_function_identity_arguments(p.oid) AS identity_args, \
                pg_catalog.pg_get_function_result(p.oid) AS return_type, \
                l.lanname AS language, \
                pg_catalog.pg_get_functiondef(p.oid) AS definition, \
                p.prokind \
         FROM pg_catalog.pg_proc p \
         JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace \
         JOIN pg_catalog.pg_language l ON l.oid = p.prolang \
         WHERE n.nspname = $1 \
           AND p.prokind IN ('f', 'p') \
           AND l.lanname != 'c' \
         ORDER BY p.proname, identity_args",
    )
    .bind(schema_name)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    func_rows
        .iter()
        .map(|r| {
            let func_name: String = r.try_get("func_name").unwrap_or_default();
            let identity_args: String = r.try_get("identity_args").unwrap_or_default();
            let signature = if identity_args.is_empty() {
                format!("{}()", func_name)
            } else {
                format!("{}({})", func_name, identity_args)
            };
            let prokind: String = r.try_get::<String, _>("prokind").unwrap_or_default();
            FunctionNode {
                id: uuid::Uuid::new_v4(),
                name: func_name,
                identity_args,
                signature,
                return_type: r.try_get("return_type").unwrap_or_default(),
                language: r.try_get("language").unwrap_or_default(),
                definition: r.try_get("definition").unwrap_or_default(),
                is_procedure: prokind == "p",
            }
        })
        .collect()
}
