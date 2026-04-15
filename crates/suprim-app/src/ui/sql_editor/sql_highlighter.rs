/// SQL syntax highlighter — re-exports from `suprim-core`.
///
/// The actual implementation now lives in `suprim_core::sql_highlighter`
/// so the premium crate can also use it for DDL preview rendering.
pub use suprim_core::sql_highlighter::sql_layout_job;
