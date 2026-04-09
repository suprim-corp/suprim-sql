//! Step 2: Compare — fetch schemas from both endpoints and compute diff.

pub(crate) mod ddl_generator;
pub(crate) mod ddl_table_helpers;
pub(crate) mod diff_engine;
pub(crate) mod diff_functions;
pub(crate) mod diff_tables;
pub(crate) mod diff_views_sequences;
