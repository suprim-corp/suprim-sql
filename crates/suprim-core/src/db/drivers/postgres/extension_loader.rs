//! Load installed extensions from a PostgreSQL database.

use sqlx::postgres::PgPool;
use sqlx::Row;

use crate::db::schema::ExtensionInfo;

/// Load all user-installed extensions for a database from `pg_extension`.
/// Excludes `plpgsql` (always present) to reduce noise.
pub(super) async fn load_extensions(pool: &PgPool) -> Vec<ExtensionInfo> {
    let rows = sqlx::query(
        "SELECT e.extname AS name, e.extversion AS version \
         FROM pg_catalog.pg_extension e \
         WHERE e.extname != 'plpgsql' \
         ORDER BY e.extname",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.iter()
        .map(|r| ExtensionInfo {
            name: r.try_get("name").unwrap_or_default(),
            version: r.try_get("version").unwrap_or_default(),
        })
        .collect()
}
