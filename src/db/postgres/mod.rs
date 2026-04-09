// src/db/postgres/mod.rs
//
// PostgreSQL driver — split into submodules:
//   driver_impl     — DatabaseDriver trait implementation
//   driver_tests    — unit tests
//   connection_url  — URL building + percent-encoding
//   type_mapping    — pg_value_from_row + rows_to_query_result
//   schema_loader   — list_databases, list_schemas, load_schema_detail
//   function_loader — load functions/procedures from pg_proc
//   queries         — execute, execute_with_params, table_data, insert/update/delete

mod connection_url;
mod driver_impl;
mod driver_tests;
mod function_loader;
mod queries;
mod schema_loader;
mod type_mapping;

pub use connection_url::{build_connection_url, urlencoding_simple};
pub use type_mapping::{pg_value_from_row, rows_to_query_result};

use std::collections::HashMap;
use std::sync::Mutex;

use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};

use crate::error::{AppError, Result};

// ─── Driver ──────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct PostgresDriver {
    /// Primary pool (for the database specified in connection config).
    pool: Option<PgPool>,
    /// Per-database pools for cross-database browsing (behind Mutex for interior mutability).
    db_pools: Mutex<HashMap<String, PgPool>>,
    /// Connection options needed to create per-database pools.
    connect_opts: Option<PgConnectOptions>,
}

impl PostgresDriver {
    pub fn new() -> Self {
        Self {
            pool: None,
            db_pools: Mutex::new(HashMap::new()),
            connect_opts: None,
        }
    }

    pub(crate) fn pool(&self) -> Result<&PgPool> {
        self.pool.as_ref().ok_or(AppError::NotConnected)
    }

    /// Get or create a pool for a specific database.
    pub(crate) async fn pool_for_db(&self, database: &str) -> Result<PgPool> {
        // Check if pool already exists.
        {
            let pools = self.db_pools.lock().unwrap();
            if let Some(pool) = pools.get(database) {
                return Ok(pool.clone());
            }
        }
        // Create new pool for this database.
        let base_opts = self
            .connect_opts
            .as_ref()
            .ok_or(AppError::NotConnected)?
            .clone();
        let opts = base_opts.database(database);
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect_with(opts)
            .await
            .map_err(|e| AppError::connection(e.to_string()))?;
        {
            let mut pools = self.db_pools.lock().unwrap();
            pools.insert(database.to_string(), pool.clone());
        }
        Ok(pool)
    }
}

impl Default for PostgresDriver {
    fn default() -> Self {
        Self::new()
    }
}
