pub mod connection;
pub mod driver;
pub mod factory;
pub mod postgres;
pub mod types;
pub mod worker;

// ── Drivers planned for future releases ──────────────────────────────────────
// pub mod sqlite;
// pub mod mysql;
// pub mod redis_driver;
// pub mod mongodb_driver;
// pub mod mssql;

pub use connection::{ConnectionConfig, DriverParams, DriverType, SshConfig, TlsConfig};
pub use driver::{DatabaseDriver, DbCommand, DbEvent};
pub use factory::DbFactory;
pub use worker::DbWorker;
pub use types::{
    ColumnMeta, ColumnNode, DatabaseNode, DbValue, ForeignKeyNode, IndexNode, QueryResult,
    SchemaNode, SchemaTree, TableNode, ViewNode,
};
