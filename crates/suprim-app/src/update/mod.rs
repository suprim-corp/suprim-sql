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

/// Base URL of the update feed. Override at build time via env var for
/// staging / local dev: `SUPRIM_UPDATE_ENDPOINT=http://localhost:8080/update/latest`.
pub const DEFAULT_ENDPOINT: &str = "https://api.sant1ago.dev/suprim/update/latest";

/// Current version baked in at compile time (matches `Cargo.toml`).
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
