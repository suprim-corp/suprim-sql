//! Schema comparison logic and DDL script generation (placeholder).

use crate::ui::dialog::tool::structure_sync::state::StructureSyncDialog;
use crate::ui::dialog::tool::structure_sync::types::DiffKind;

impl StructureSyncDialog {
    pub(crate) fn run_comparison(&mut self) {
        self.compared = true;
        self.diff_entries.clear();
        self.ddl_script.clear();
        self.status = None;

        if self.source.database.is_empty() || self.target.database.is_empty() {
            self.status = Some("Please select a database for both source and target.".into());
            self.compared = false;
            return;
        }
        if self.source.schema.is_empty() || self.target.schema.is_empty() {
            self.status = Some("Please select a schema for both source and target.".into());
            self.compared = false;
            return;
        }

        let src = match self.connections.get(self.source.conn_idx) {
            Some(c) => c,
            None => {
                self.status = Some("Invalid source connection.".into());
                self.compared = false;
                return;
            }
        };
        let tgt = match self.connections.get(self.target.conn_idx) {
            Some(c) => c,
            None => {
                self.status = Some("Invalid target connection.".into());
                self.compared = false;
                return;
            }
        };

        if src.conn_id == tgt.conn_id
            && self.source.database == self.target.database
            && self.source.schema == self.target.schema
        {
            self.status = Some("Source and target are the same schema.".into());
            self.compared = false;
            return;
        }

        // TODO: Real comparison via async schema fetching.
        self.status = Some(format!(
            "Comparison {}/{}/{} {} {}/{}/{} — coming soon.",
            src.label,
            self.source.database,
            self.source.schema,
            egui_phosphor::regular::ARROW_RIGHT,
            tgt.label,
            self.target.database,
            self.target.schema,
        ));
    }

    #[allow(dead_code)]
    pub(crate) fn regenerate_script(&mut self) {
        let mut lines = Vec::new();
        for entry in &self.diff_entries {
            if !entry.checked {
                continue;
            }
            match entry.kind {
                DiffKind::Added => lines.push(format!("-- + {}", entry.label)),
                DiffKind::Removed => lines.push(format!("-- - {}", entry.label)),
                DiffKind::Modified => lines.push(format!("-- \u{0394} {}", entry.label)),
            }
        }
        self.ddl_script = lines.join("\n");
    }
}
