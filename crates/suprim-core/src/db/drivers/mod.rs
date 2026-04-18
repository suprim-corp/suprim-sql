//! Database driver implementations — one submodule per engine.

pub mod postgres;
pub mod mysql;

// ── Drivers planned for future releases ──────────────────────────────────────
// pub mod sqlite;
// pub mod redis;
//
// MongoDB and MSSQL drivers are in the private suprim-extensions crate.
