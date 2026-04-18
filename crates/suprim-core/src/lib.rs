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
    /// App version (pulled from Cargo.toml at compile time).
    pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
    /// Base website URL.
    pub const WEB_URL: &str = "https://suprim.dev";
    /// GitHub releases URL.
    pub const RELEASES_URL: &str = "https://github.com/suprim-corp/suprim-sql/releases";
    /// Base API URL (overridable via `SUPRIM_API_URL` env var).
    pub fn api_url() -> String {
        std::env::var("SUPRIM_API_URL").unwrap_or_else(|_| format!("{WEB_URL}/api"))
    }
}
