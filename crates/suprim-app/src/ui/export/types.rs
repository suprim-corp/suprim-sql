//! Shared types for the export dialog.

use std::path::PathBuf;
use uuid::Uuid;

use suprim_core::db::values::QueryResult;

/// Single table/view in the export tree.
#[derive(Debug, Clone)]
pub struct ExportTableItem {
    pub name: String,
    pub database: String,
    pub schema: String,
    pub is_view: bool,
    pub selected: bool,
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
}

impl ExportFormatId {
    pub fn all() -> &'static [ExportFormatId] {
        &[ExportFormatId::Csv, ExportFormatId::Json]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ExportFormatId::Csv => "CSV",
            ExportFormatId::Json => "JSON",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormatId::Csv => "csv",
            ExportFormatId::Json => "json",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ExportFormatId::Csv => "Comma-separated values. Compatible with Excel and most tools.",
            ExportFormatId::Json => "JSON array of objects. One element per row.",
        }
    }
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
    pub format: ExportFormatId,
    /// Output path. For query results: a file. For table mode: a file (if 1 table)
    /// or directory where one file per table will be written.
    pub destination: PathBuf,
    /// Tables selected (only relevant for Tables mode).
    pub selected_tables: Vec<SelectedTable>,
    /// Format-specific options.
    pub csv_options: crate::ui::export::csv_plugin::CsvOptions,
    pub json_options: crate::ui::export::json_plugin::JsonOptions,
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
}
