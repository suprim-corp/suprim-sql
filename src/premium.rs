//! Premium feature bridge — runtime feature gating for Open Core model.
//!
//! Defines the `PremiumGate` trait used throughout the app for feature gating.
//! The free-tier stub (`FreeTierGate`) is always available.
//! The actual premium implementation lives in the `suprim-premium` private crate
//! and is wired in via `main.rs` when built with `--features premium`.

use crate::db::connection::{ConnectionConfig, DriverType};
use crate::db::driver::DatabaseDriver;
use crate::error::Result;

/// Trait for premium feature gating — implemented by both the free stub
/// and the real `suprim-premium` crate.
pub trait PremiumGate: Send + Sync + std::fmt::Debug {
    /// Display name for the current tier ("Free" / "Premium").
    fn tier_name(&self) -> &str;

    /// Check if a driver type is allowed on the current tier.
    fn can_use_driver(&self, driver: &DriverType) -> std::result::Result<(), String>;

    /// Check if adding a new connection is allowed.
    fn can_add_connection(&self, current_count: usize) -> std::result::Result<(), String>;

    /// Try to create a premium-only driver. Returns `None` if the driver type
    /// is not a premium driver (fall through to built-in factory).
    fn create_driver(&self, config: &ConnectionConfig) -> Option<Result<Box<dyn DatabaseDriver>>>;

    /// Maximum number of connections allowed, or `None` for unlimited (Premium).
    fn connection_limit(&self) -> Option<usize>;

    /// Whether the gate has a license key that needs online validation.
    fn needs_validation(&self) -> bool;

    /// Validate the license online (async). No-op for free tier.
    fn validate_online(
        &mut self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = std::result::Result<(), String>> + Send + '_>,
    >;
}

// ── Free tier stub (always compiled) ──────────────────────────────────────────

const FREE_MAX_CONNECTIONS: usize = 5;

#[derive(Debug)]
pub struct FreeTierGate;

impl PremiumGate for FreeTierGate {
    fn tier_name(&self) -> &str {
        "Free"
    }

    fn can_use_driver(&self, driver: &DriverType) -> std::result::Result<(), String> {
        match driver {
            DriverType::MongoDB => Err("MongoDB requires Premium plan. Upgrade to unlock.".into()),
            DriverType::Mssql => {
                Err("SQL Server requires Premium plan. Upgrade to unlock.".into())
            }
            _ => Ok(()),
        }
    }

    fn can_add_connection(&self, current_count: usize) -> std::result::Result<(), String> {
        if current_count >= FREE_MAX_CONNECTIONS {
            Err(format!(
                "Free plan supports up to {} connections. Upgrade to Premium for unlimited.",
                FREE_MAX_CONNECTIONS
            ))
        } else {
            Ok(())
        }
    }

    fn create_driver(
        &self,
        _config: &ConnectionConfig,
    ) -> Option<Result<Box<dyn DatabaseDriver>>> {
        None
    }

    fn connection_limit(&self) -> Option<usize> {
        Some(FREE_MAX_CONNECTIONS)
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
}

/// Create the free tier gate (used when premium feature is not enabled).
pub fn create_free_gate() -> Box<dyn PremiumGate> {
    Box::new(FreeTierGate)
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_gate_tier_name() {
        let gate = FreeTierGate;
        assert_eq!(gate.tier_name(), "Free");
    }

    #[test]
    fn free_gate_allows_postgres() {
        let gate = FreeTierGate;
        assert!(gate.can_use_driver(&DriverType::Postgres).is_ok());
    }

    #[test]
    fn free_gate_blocks_mongodb() {
        let gate = FreeTierGate;
        assert!(gate.can_use_driver(&DriverType::MongoDB).is_err());
    }

    #[test]
    fn free_gate_blocks_mssql() {
        let gate = FreeTierGate;
        assert!(gate.can_use_driver(&DriverType::Mssql).is_err());
    }

    #[test]
    fn free_gate_connection_limit() {
        let gate = FreeTierGate;
        assert!(gate.can_add_connection(0).is_ok());
        assert!(gate.can_add_connection(4).is_ok());
        assert!(gate.can_add_connection(5).is_err());
    }

    #[test]
    fn free_gate_no_premium_drivers() {
        let gate = FreeTierGate;
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
    fn free_gate_no_validation_needed() {
        let gate = FreeTierGate;
        assert!(!gate.needs_validation());
    }
}
