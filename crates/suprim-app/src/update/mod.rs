//! Self-update: poll the suprim-server feed, compare versions, download the
//! DMG, verify its SHA-256, and swap the running .app atomically.
//!
//! Workflow overview:
//!
//! 1. [`UpdateChecker::check`] hits `/suprim/update/latest` and returns a
//!    [`LatestRelease`] if the server-reported version is newer than the
//!    running version (semver compare).
//! 2. UI surfaces an "Update available" banner with the release notes + a
//!    "Install update" button.
//! 3. Clicking the button spawns [`install::run`] which downloads the DMG,
//!    hashes it, mounts it, copies the .app into `/Applications/`, unmounts,
//!    and relaunches the new binary.

mod check;
mod install;
pub mod state;

pub use check::{check_for_update, LatestRelease};
pub use install::install_update;
pub use state::{UpdateProgress, UpdateState};

/// Base URL of the update feed.
///
/// Resolved in this order at runtime, first hit wins:
///
/// 1. Runtime env var `SUPRIM_UPDATE_ENDPOINT` — local dev override.
/// 2. Build-time env var `SUPRIM_UPDATE_ENDPOINT` baked in via `option_env!`
///    at compile time. Set it in the release pipeline so different build
///    targets (staging vs prod, beta vs stable) can ship with different
///    defaults without editing source.
/// 3. The hardcoded fallback below — production feed on the corp domain.
///
/// Build with a non-default default:
///
/// ```sh
/// SUPRIM_UPDATE_ENDPOINT=https://staging.api.suprim.dev/suprim/update/latest \
///   cargo build --release
/// ```
pub const DEFAULT_ENDPOINT: &str = match option_env!("SUPRIM_UPDATE_ENDPOINT") {
    Some(s) => s,
    None => "https://api.suprim.dev/suprim/update/latest",
};

/// Current version baked in at compile time (matches `Cargo.toml`).
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
