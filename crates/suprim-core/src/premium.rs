//! Premium feature bridge — runtime feature gating for Open Core model.
//!
//! Defines the `PremiumGate` trait used throughout the app for feature gating.
//! The development stub (`DevGate`) is always available — it imposes no limits.
//! The actual premium implementation (with license enforcement and limits)
//! lives in the `suprim-premium` private crate and is wired in via `main.rs`
//! when built with `--features premium`.

use crate::db::connection::{ConnectionConfig, DriverType};
use crate::db::driver::DatabaseDriver;
use crate::error::Result;

/// Trait for premium feature gating — implemented by both the dev stub
/// and the real `suprim-premium` crate.
pub trait PremiumGate: Send + Sync + std::fmt::Debug {
    /// Display name for the current tier (e.g. "Free", "Premium", "Development").
    fn tier_name(&self) -> &str;

    /// Check if a driver type is allowed on the current tier.
    fn can_use_driver(&self, driver: &DriverType) -> std::result::Result<(), String>;

    /// Check if adding a new connection is allowed.
    fn can_add_connection(&self, current_count: usize) -> std::result::Result<(), String>;

    /// Try to create a premium-only driver. Returns `None` if the driver type
    /// is not a premium driver (fall through to built-in factory).
    fn create_driver(&self, config: &ConnectionConfig) -> Option<Result<Box<dyn DatabaseDriver>>>;

    /// Maximum number of connections allowed, or `None` for unlimited.
    fn connection_limit(&self) -> Option<usize>;

    /// Whether the gate has a license key that needs online validation.
    fn needs_validation(&self) -> bool;

    /// Validate the license online (async). No-op for dev gate.
    fn validate_online(
        &mut self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>,
    >;

    /// Signed-in user email, or `None` if not signed in.
    fn user_email(&self) -> Option<&str>;

    /// Create a Structure Sync dialog as a trait object.
    /// Returns `None` for free/dev builds (feature not available).
    #[cfg(feature = "ui")]
    fn create_structure_sync(
        &self,
        connections: Vec<crate::sync_types::ConnInfo>,
    ) -> Option<Box<dyn crate::sync_types::ToolDialog>>;
}

// ── Development stub (always compiled) ────────────────────────────────────────
// No enforcement, no limits. Used when premium crate is not linked.
// All limits and license logic live exclusively in the private `suprim-premium` crate.

#[derive(Debug)]
pub struct DevGate;

impl PremiumGate for DevGate {
    fn tier_name(&self) -> &str {
        "Development"
    }

    fn can_use_driver(&self, _driver: &DriverType) -> std::result::Result<(), String> {
        Ok(())
    }

    fn can_add_connection(&self, _current_count: usize) -> std::result::Result<(), String> {
        Ok(())
    }

    fn create_driver(
        &self,
        _config: &ConnectionConfig,
    ) -> Option<Result<Box<dyn DatabaseDriver>>> {
        None
    }

    fn connection_limit(&self) -> Option<usize> {
        None
    }

    fn needs_validation(&self) -> bool {
        false
    }

    fn validate_online(
        &mut self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>,
    > {
        Box::pin(async { Ok(()) })
    }

    fn user_email(&self) -> Option<&str> {
        None
    }

    #[cfg(feature = "ui")]
    fn create_structure_sync(
        &self,
        _connections: Vec<crate::sync_types::ConnInfo>,
    ) -> Option<Box<dyn crate::sync_types::ToolDialog>> {
        None // Structure Sync not available in dev/free builds
    }
}

/// Create the development gate (used when premium feature is not enabled).
pub fn create_free_gate() -> Box<dyn PremiumGate> {
    Box::new(DevGate)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_gate_tier_name() {
        let gate = DevGate;
        assert_eq!(gate.tier_name(), "Development");
    }

    #[test]
    fn dev_gate_allows_all_drivers() {
        let gate = DevGate;
        assert!(gate.can_use_driver(&DriverType::Postgres).is_ok());
        assert!(gate.can_use_driver(&DriverType::MongoDB).is_ok());
        assert!(gate.can_use_driver(&DriverType::Mssql).is_ok());
    }

    #[test]
    fn dev_gate_unlimited_connections() {
        let gate = DevGate;
        assert!(gate.can_add_connection(0).is_ok());
        assert!(gate.can_add_connection(100).is_ok());
        assert!(gate.connection_limit().is_none());
    }

    #[test]
    fn dev_gate_no_premium_drivers() {
        let gate = DevGate;
        let config = ConnectionConfig::new(
            "test",
            crate::db::connection::DriverParams::Postgres {
                host: "localhost".into(),
                port: 5432,
                database: "db".into(),
                user: "user".into(),
                password_key: "key".into(),
            },
        );
        assert!(gate.create_driver(&config).is_none());
    }

    #[test]
    fn dev_gate_no_validation_needed() {
        let gate = DevGate;
        assert!(!gate.needs_validation());
    }
}
