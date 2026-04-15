pub mod db;
pub mod error;
pub mod premium;
pub mod storage;

#[cfg(feature = "ui")]
pub mod sql_highlighter;

#[cfg(feature = "ui")]
pub mod sync_types;

/// App-wide constants — single source of truth for URLs, limits, etc.
pub mod constants {
    /// Base website URL.
    pub const WEB_URL: &str = "https://suprim.dev";
    /// Base API URL (overridable via `SUPRIM_API_URL` env var).
    pub fn api_url() -> String {
        std::env::var("SUPRIM_API_URL").unwrap_or_else(|_| format!("{WEB_URL}/api"))
    }
}
