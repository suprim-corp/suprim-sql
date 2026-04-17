//! Validation + export request construction (native save dialog).

use super::types::{ExportMode, ExportModeKind, ExportRequest, SelectedTable};
use super::ExportDialog;

impl ExportDialog {
    pub(super) fn selected_count(&self) -> usize {
        match &self.mode {
            ExportMode::Tables { items, .. } => items
                .iter()
                .flat_map(|d| d.schemas.iter())
                .flat_map(|s| s.tables.iter())
                .filter(|t| t.selected)
                .count(),
            ExportMode::QueryResult { .. } => 1,
        }
    }

    /// Validate state and update `self.error`. Returns true if export is allowed.
    pub(super) fn validate_state(&mut self) -> bool {
        // Unavailable formats (SQL, XLSX) are always disabled.
        if !self.format.is_available() {
            self.error = None;
            return false;
        }
        let trimmed = self.file_name.trim();
        if trimmed.is_empty() {
            self.error = None; // Empty is a silent disable, not an error
            return false;
        }
        if let Some(c) = self
            .file_name
            .chars()
            .find(|c| matches!(*c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        {
            self.error = Some(format!("Invalid character in file name: '{c}'"));
            return false;
        }
        self.error = None;
        match &self.mode {
            ExportMode::Tables { .. } => self.selected_count() > 0,
            ExportMode::QueryResult { .. } => true,
        }
    }

    /// Open the native save-file/folder dialog and build an `ExportRequest`.
    /// Returns `None` if the user cancels the native picker.
    pub(super) fn build_request(&mut self) -> Option<ExportRequest> {
        let (destination, selected_tables, mode_kind, query_result) = match &mut self.mode {
            ExportMode::QueryResult { result, .. } => {
                let path = rfd::FileDialog::new()
                    .set_file_name(format!("{}.{}", self.file_name, self.format.extension()))
                    .add_filter(self.format.label(), &[self.format.extension()])
                    .save_file()?;
                (
                    path,
                    Vec::new(),
                    ExportModeKind::QueryResult,
                    Some(result.clone()),
                )
            }
            ExportMode::Tables { conn_id, items } => {
                let selected: Vec<SelectedTable> = items
                    .iter()
                    .flat_map(|d| d.schemas.iter())
                    .flat_map(|s| s.tables.iter())
                    .filter(|t| t.selected)
                    .map(|t| SelectedTable {
                        conn_id: *conn_id,
                        database: t.database.clone(),
                        schema: t.schema.clone(),
                        name: t.name.clone(),
                    })
                    .collect();

                let path = if selected.len() == 1 {
                    rfd::FileDialog::new()
                        .set_file_name(format!("{}.{}", self.file_name, self.format.extension()))
                        .add_filter(self.format.label(), &[self.format.extension()])
                        .save_file()?
                } else {
                    rfd::FileDialog::new()
                        .set_title("Choose output directory (one file per table)")
                        .pick_folder()?
                };
                (path, selected, ExportModeKind::Tables, None)
            }
        };

        Some(ExportRequest {
            mode_kind,
            format: self.format,
            destination,
            selected_tables,
            csv_options: self.csv_opts.clone(),
            json_options: self.json_opts.clone(),
            query_result,
        })
    }
}
