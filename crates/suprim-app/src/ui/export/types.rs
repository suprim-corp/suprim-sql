//! Shared types for the export dialog.

use std::path::PathBuf;
use uuid::Uuid;

use suprim_core::db::values::QueryResult;
use suprim_core::db::TableNode;

use super::csv_options::CsvOptions;
use super::json_options::JsonOptions;
use super::sql_options::SqlOptions;

/// Single table/view in the export tree.
#[derive(Debug, Clone)]
pub struct ExportTableItem {
    pub name: String,
    pub database: String,
    pub schema: String,
    pub is_view: bool,
    pub selected: bool,
    // ── Per-table options (used only when format supports them, e.g. SQL) ──
    pub sql_include_structure: bool,
    pub sql_include_drop: bool,
    pub sql_include_data: bool,
}

impl ExportTableItem {
    pub fn new(name: String, database: String, schema: String, is_view: bool) -> Self {
        Self {
            name,
            database,
            schema,
            is_view,
            selected: false,
            sql_include_structure: true,
            sql_include_drop: false,
            sql_include_data: true,
        }
    }
}

/// Schema group in the export tree (contains tables).
#[derive(Debug, Clone)]
pub struct ExportSchemaItem {
    pub name: String,
    pub database: String,
    pub tables: Vec<ExportTableItem>,
    pub expanded: bool,
}

/// Database group in the export tree.
#[derive(Debug, Clone)]
pub struct ExportDatabaseItem {
    pub name: String,
    pub schemas: Vec<ExportSchemaItem>,
    pub expanded: bool,
}

/// Selected format id (persisted across dialog opens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormatId {
    Csv,
    Json,
    Sql,
    Xlsx,
}

impl ExportFormatId {
    pub fn all() -> &'static [ExportFormatId] {
        &[
            ExportFormatId::Csv,
            ExportFormatId::Json,
            ExportFormatId::Sql,
            ExportFormatId::Xlsx,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ExportFormatId::Csv => "CSV",
            ExportFormatId::Json => "JSON",
            ExportFormatId::Sql => "SQL",
            ExportFormatId::Xlsx => "XLSX (Pro)",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormatId::Csv => "csv",
            ExportFormatId::Json => "json",
            ExportFormatId::Sql => "sql",
            ExportFormatId::Xlsx => "xlsx",
        }
    }

    /// Whether this format is fully implemented. Disabled formats show "Coming soon".
    pub fn is_available(&self) -> bool {
        matches!(
            self,
            ExportFormatId::Csv | ExportFormatId::Json | ExportFormatId::Sql
        )
    }

    pub fn description(&self) -> &'static str {
        match self {
            ExportFormatId::Csv => "Comma-separated values. Compatible with Excel and most tools.",
            ExportFormatId::Json => "JSON array of objects. One element per row.",
            ExportFormatId::Sql => "SQL INSERT statements. Replay into any SQL database.",
            ExportFormatId::Xlsx => "Excel workbook. One sheet per table, with formatting.",
        }
    }
}

// ── FormatOptions ───────────────────────────────────────────────────────────

/// Active export format with its associated options.
pub enum FormatOptions {
    Csv(CsvOptions),
    Json(JsonOptions),
    Sql(SqlOptions),
}

impl FormatOptions {
    pub fn format_id(&self) -> ExportFormatId {
        match self {
            FormatOptions::Csv(_) => ExportFormatId::Csv,
            FormatOptions::Json(_) => ExportFormatId::Json,
            FormatOptions::Sql(_) => ExportFormatId::Sql,
        }
    }

    pub fn is_gzip(&self) -> bool {
        match self {
            FormatOptions::Csv(o) => o.gzip,
            FormatOptions::Json(o) => o.gzip,
            FormatOptions::Sql(o) => o.gzip,
        }
    }

    pub fn extension(&self) -> String {
        let base = self.format_id().extension();
        if self.is_gzip() {
            format!("{base}.gz")
        } else {
            base.to_string()
        }
    }
}

// ── SqlExportInfo ───────────────────────────────────────────────────────────

/// Per-table metadata passed to the SQL writer via `execute_export`.
pub struct SqlExportInfo<'a> {
    pub schema: &'a str,
    pub table_name: &'a str,
    pub include_structure: bool,
    pub include_drop: bool,
    pub include_data: bool,
    pub table_node: Option<&'a TableNode>,
}

/// Mode the dialog was opened in.
pub enum ExportMode {
    /// Export selected tables from the sidebar (requires fetching data).
    Tables {
        conn_id: Uuid,
        items: Vec<ExportDatabaseItem>,
    },
    /// Export an already-loaded query result (from the table viewer or SQL editor).
    QueryResult { result: QueryResult },
}

/// What the dialog returns when user clicks Export.
pub enum ExportOutcome {
    /// Dialog still open.
    Pending,
    /// User cancelled.
    Cancelled,
    /// User chose format + destination. App should perform the export.
    Export(ExportRequest),
}

/// Request to perform an export.
pub struct ExportRequest {
    pub mode_kind: ExportModeKind,
    /// Output path. For query results: a file. For table mode: a file (if 1 table)
    /// or directory where one file per table will be written.
    pub destination: PathBuf,
    /// Tables selected (only relevant for Tables mode).
    pub selected_tables: Vec<SelectedTable>,
    /// Format-specific options (active format + its config).
    pub format_options: FormatOptions,
    /// For QueryResult mode — the already-loaded result to write directly.
    pub query_result: Option<QueryResult>,
}

/// What mode triggered the export — tells the app handler how to proceed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportModeKind {
    Tables,
    QueryResult,
}

/// A table to fetch and export.
#[derive(Debug, Clone)]
pub struct SelectedTable {
    pub conn_id: Uuid,
    pub database: String,
    pub schema: String,
    pub name: String,
    /// Per-table SQL toggles (only used when format is SQL).
    pub sql_include_structure: bool,
    pub sql_include_drop: bool,
    pub sql_include_data: bool,
}

// ── PendingExport ───────────────────────────────────────────────────────────

/// A pending export waiting for query results to come back (Tables mode).
pub struct PendingExport {
    pub destination: PathBuf,
    pub format_options: FormatOptions,
    pub table_name: String,
    pub schema: String,
    pub sql_include_structure: bool,
    pub sql_include_drop: bool,
    pub sql_include_data: bool,
    /// Full table metadata for DDL generation (populated from sidebar schema tree).
    pub table_node: Option<TableNode>,
}
