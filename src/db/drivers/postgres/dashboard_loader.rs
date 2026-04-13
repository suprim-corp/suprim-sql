//! Load active sessions, slow queries, and server metrics from PostgreSQL catalog views.

use sqlx::postgres::PgPool;
use sqlx::Row;

use crate::db::schema::{ServerMetrics, SessionInfo, SlowQueryInfo};

/// Load active (non-idle) sessions from `pg_stat_activity`.
pub(super) async fn load_sessions(pool: &PgPool) -> Vec<SessionInfo> {
    let rows = sqlx::query(
        "SELECT pid, \
                COALESCE(usename, '') AS usename, \
                COALESCE(datname, '') AS datname, \
                COALESCE(state, '') AS state, \
                COALESCE(EXTRACT(EPOCH FROM (now() - query_start))::text, '') AS duration_secs, \
                COALESCE(query, '') AS query \
         FROM pg_stat_activity \
         WHERE pid != pg_backend_pid() \
           AND state IS NOT NULL \
           AND state != '' \
         ORDER BY query_start ASC NULLS LAST",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.iter()
        .map(|r| {
            let secs_str: String = r.try_get("duration_secs").unwrap_or_default();
            let duration = format_duration_secs(&secs_str);
            SessionInfo {
                pid: r.try_get("pid").unwrap_or(0),
                user: r.try_get("usename").unwrap_or_default(),
                database: r.try_get("datname").unwrap_or_default(),
                state: r.try_get("state").unwrap_or_default(),
                duration,
                query: r.try_get("query").unwrap_or_default(),
            }
        })
        .collect()
}

/// Load server-level metrics from various PostgreSQL catalog views.
pub(super) async fn load_metrics(pool: &PgPool) -> ServerMetrics {
    // Core metrics — no optional extensions, always available
    let row = sqlx::query(
        "SELECT \
            (SELECT count(*) FROM pg_stat_activity) AS connected, \
            (SELECT count(*) FROM pg_stat_activity WHERE state = 'active') AS active, \
            (SELECT EXTRACT(EPOCH FROM now() - pg_postmaster_start_time())::bigint) AS uptime_secs, \
            (SELECT COALESCE(sum(xact_commit + xact_rollback), 0) FROM pg_stat_database) AS total_xact, \
            (SELECT setting::bigint FROM pg_settings WHERE name = 'max_connections') AS max_conn, \
            (SELECT pg_size_pretty(COALESCE(sum(blks_read)::bigint * 8192, 0)) FROM pg_stat_database) AS bytes_recv, \
            (SELECT pg_size_pretty(COALESCE(sum(blks_hit)::bigint * 8192, 0)) FROM pg_stat_database) AS bytes_sent",
    )
    .fetch_one(pool)
    .await;

    let mut metrics = match row {
        Ok(r) => {
            let uptime_secs: i64 = r.try_get("uptime_secs").unwrap_or(0);
            ServerMetrics {
                connected_sessions: r.try_get("connected").unwrap_or(0),
                active_queries: r.try_get("active").unwrap_or(0),
                uptime: format_uptime(uptime_secs),
                total_transactions: r.try_get("total_xact").unwrap_or(0),
                slow_queries: 0,
                max_connections: r.try_get("max_conn").unwrap_or(0),
                bytes_received: r.try_get("bytes_recv").unwrap_or_default(),
                bytes_sent: r.try_get("bytes_sent").unwrap_or_default(),
            }
        }
        Err(_) => ServerMetrics::default(),
    };

    // Slow query count — optional, requires pg_stat_statements extension
    if let Ok(row) = sqlx::query(
        "SELECT count(*) AS cnt FROM pg_stat_statements WHERE mean_exec_time > 1000",
    )
    .fetch_one(pool)
    .await
    {
        metrics.slow_queries = row.try_get("cnt").unwrap_or(0);
    }

    metrics
}

/// Terminate a backend process by PID.
pub(super) async fn kill_session(pool: &PgPool, pid: i32) -> crate::error::Result<()> {
    sqlx::query("SELECT pg_terminate_backend($1)")
        .bind(pid)
        .execute(pool)
        .await
        .map_err(|e| crate::error::AppError::query("pg_terminate_backend", e.to_string()))?;
    Ok(())
}

/// Load top slow queries from `pg_stat_statements` (requires extension).
/// Returns empty vec if the extension is not installed.
pub(super) async fn load_slow_queries(pool: &PgPool) -> Vec<SlowQueryInfo> {
    // Check if pg_stat_statements extension is available
    let rows = sqlx::query(
        "SELECT query, calls, total_exec_time, mean_exec_time, max_exec_time, rows \
         FROM pg_stat_statements \
         WHERE mean_exec_time > 100 \
         ORDER BY mean_exec_time DESC \
         LIMIT 20",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.iter()
        .map(|r| SlowQueryInfo {
            query: r.try_get("query").unwrap_or_default(),
            calls: r.try_get("calls").unwrap_or(0),
            total_time_ms: r.try_get("total_exec_time").unwrap_or(0.0),
            mean_time_ms: r.try_get("mean_exec_time").unwrap_or(0.0),
            max_time_ms: r.try_get("max_exec_time").unwrap_or(0.0),
            rows: r.try_get("rows").unwrap_or(0),
        })
        .collect()
}

/// Format seconds into human-readable duration (e.g. "5d 2h 30m").
fn format_uptime(total_secs: i64) -> String {
    if total_secs <= 0 {
        return "0s".to_string();
    }
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let mins = (total_secs % 3600) / 60;
    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if mins > 0 || parts.is_empty() {
        parts.push(format!("{mins}m"));
    }
    parts.join(" ")
}

/// Format duration_secs string from pg into human-readable (e.g. "7s", "2m 15s").
fn format_duration_secs(secs_str: &str) -> String {
    let secs: f64 = secs_str.parse().unwrap_or(0.0);
    if secs < 0.001 {
        return String::new();
    }
    let total = secs as i64;
    if total < 60 {
        format!("{total}s")
    } else if total < 3600 {
        format!("{}m {}s", total / 60, total % 60)
    } else {
        format!("{}h {}m", total / 3600, (total % 3600) / 60)
    }
}
