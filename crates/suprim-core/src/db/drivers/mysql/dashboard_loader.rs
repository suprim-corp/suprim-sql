//! Load active sessions, slow queries, and server metrics from MySQL.
//!
//! Uses `information_schema.processlist`, `SHOW GLOBAL STATUS`, `SHOW VARIABLES`,
//! and `performance_schema.events_statements_summary_by_digest`.

use sqlx::mysql::MySqlPool;
use sqlx::{AssertSqlSafe, Row};

use crate::db::schema::{ServerMetrics, SessionInfo, SlowQueryInfo};

/// Load active sessions from `information_schema.processlist`.
pub(super) async fn load_sessions(pool: &MySqlPool) -> Vec<SessionInfo> {
    let rows = sqlx::query(
        "SELECT ID, USER, HOST, COALESCE(DB, '') AS DB, \
                COALESCE(COMMAND, '') AS COMMAND, \
                COALESCE(TIME, 0) AS TIME, \
                COALESCE(STATE, '') AS STATE, \
                COALESCE(INFO, '') AS INFO \
         FROM information_schema.processlist \
         ORDER BY TIME DESC",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.iter()
        .map(|r| {
            let time_secs: i64 = r.try_get::<i64, _>("TIME").unwrap_or(0);
            SessionInfo {
                pid: r.try_get::<i64, _>("ID").unwrap_or(0) as i32,
                user: r.try_get("USER").unwrap_or_default(),
                database: r.try_get("DB").unwrap_or_default(),
                state: r.try_get("STATE").unwrap_or_default(),
                duration: format_duration_secs(time_secs),
                query: r.try_get("INFO").unwrap_or_default(),
            }
        })
        .collect()
}

/// Load server-level metrics from `SHOW GLOBAL STATUS` and `SHOW VARIABLES`.
pub(super) async fn load_metrics(pool: &MySqlPool) -> ServerMetrics {
    // Collect status variables into a map
    let status_rows = sqlx::query(
        "SHOW GLOBAL STATUS WHERE Variable_name IN (\
            'Uptime', 'Threads_connected', 'Threads_running', \
            'Queries', 'Slow_queries', 'Bytes_sent', 'Bytes_received', 'Connections'\
        )",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut status: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for r in &status_rows {
        let name: String = r.try_get(0).unwrap_or_default();
        let value: String = r.try_get(1).unwrap_or_default();
        status.insert(name, value);
    }

    // Collect variables into a map
    let var_rows = sqlx::query(
        "SHOW VARIABLES WHERE Variable_name IN ('version', 'max_connections')",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut vars: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for r in &var_rows {
        let name: String = r.try_get(0).unwrap_or_default();
        let value: String = r.try_get(1).unwrap_or_default();
        vars.insert(name, value);
    }

    let uptime_secs: i64 = status
        .get("Uptime")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let connected: i64 = status
        .get("Threads_connected")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let active: i64 = status
        .get("Threads_running")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let total_queries: i64 = status
        .get("Queries")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let slow: i64 = status
        .get("Slow_queries")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let max_conn: i64 = vars
        .get("max_connections")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let bytes_recv: u64 = status
        .get("Bytes_received")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let bytes_sent: u64 = status
        .get("Bytes_sent")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    ServerMetrics {
        connected_sessions: connected,
        active_queries: active,
        uptime: format_uptime(uptime_secs),
        total_transactions: total_queries,
        slow_queries: slow,
        max_connections: max_conn,
        bytes_received: format_bytes(bytes_recv),
        bytes_sent: format_bytes(bytes_sent),
    }
}

/// Terminate a session/process by PID.
pub(super) async fn kill_session(pool: &MySqlPool, pid: i32) -> crate::error::Result<()> {
    let sql = format!("KILL {}", pid);
    sqlx::query(AssertSqlSafe(sql.clone()))
        .execute(pool)
        .await
        .map_err(|e| crate::error::AppError::query(&sql, e.to_string()))?;
    Ok(())
}

/// Load top slow queries from `performance_schema.events_statements_summary_by_digest`.
/// Returns empty vec if `performance_schema` is not available.
pub(super) async fn load_slow_queries(pool: &MySqlPool) -> Vec<SlowQueryInfo> {
    let rows = sqlx::query(
        "SELECT DIGEST_TEXT, \
                COUNT_STAR, \
                AVG_TIMER_WAIT / 1000000000 AS avg_ms, \
                SUM_TIMER_WAIT / 1000000000 AS total_ms, \
                MAX_TIMER_WAIT / 1000000000 AS max_ms, \
                SUM_ROWS_SENT AS total_rows \
         FROM performance_schema.events_statements_summary_by_digest \
         WHERE DIGEST_TEXT IS NOT NULL \
         ORDER BY AVG_TIMER_WAIT DESC \
         LIMIT 20",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.iter()
        .map(|r| SlowQueryInfo {
            query: r.try_get("DIGEST_TEXT").unwrap_or_default(),
            calls: r.try_get::<i64, _>("COUNT_STAR").unwrap_or(0),
            total_time_ms: r.try_get::<f64, _>("total_ms").unwrap_or(0.0),
            mean_time_ms: r.try_get::<f64, _>("avg_ms").unwrap_or(0.0),
            max_time_ms: r.try_get::<f64, _>("max_ms").unwrap_or(0.0),
            rows: r.try_get::<i64, _>("total_rows").unwrap_or(0),
        })
        .collect()
}

// ─── Formatting helpers ──────────────────────────────────────────────────────

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

/// Format a TIME (seconds) value into human-readable duration.
fn format_duration_secs(secs: i64) -> String {
    if secs <= 0 {
        return String::new();
    }
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Format bytes into human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_uptime_zero() {
        assert_eq!(format_uptime(0), "0s");
    }

    #[test]
    fn format_uptime_minutes() {
        assert_eq!(format_uptime(300), "5m");
    }

    #[test]
    fn format_uptime_hours_and_minutes() {
        assert_eq!(format_uptime(3900), "1h 5m");
    }

    #[test]
    fn format_uptime_days() {
        assert_eq!(format_uptime(90061), "1d 1h 1m");
    }

    #[test]
    fn format_duration_secs_zero() {
        assert_eq!(format_duration_secs(0), "");
    }

    #[test]
    fn format_duration_secs_small() {
        assert_eq!(format_duration_secs(45), "45s");
    }

    #[test]
    fn format_duration_secs_minutes() {
        assert_eq!(format_duration_secs(135), "2m 15s");
    }

    #[test]
    fn format_duration_secs_hours() {
        assert_eq!(format_duration_secs(3661), "1h 1m");
    }

    #[test]
    fn format_bytes_small() {
        assert_eq!(format_bytes(500), "500 bytes");
    }

    #[test]
    fn format_bytes_kb() {
        assert_eq!(format_bytes(1536), "1.5 KB");
    }

    #[test]
    fn format_bytes_mb() {
        assert_eq!(format_bytes(10_485_760), "10.0 MB");
    }

    #[test]
    fn format_bytes_gb() {
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }
}
