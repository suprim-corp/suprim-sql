/// Pending changes buffer — tracks uncommitted add/edit/delete operations.
/// Changes are only sent to the database when the user clicks Execute (▲).
use std::collections::{HashMap, HashSet};

use suprim_core::db::types::DbValue;

/// A single cell edit that hasn't been committed yet.
#[derive(Clone)]
pub struct EditedCell {
    pub column_name: String,
    pub original_value: DbValue,
    pub new_value: DbValue,
}

/// A new row to be inserted (not yet committed).
#[derive(Clone)]
pub struct NewRow {
    /// Column name → value. Missing columns use DEFAULT.
    pub values: HashMap<String, DbValue>,
}

/// An undo-able action in the pending changes buffer.
#[derive(Clone)]
pub enum UndoEntry {
    /// A row was marked for deletion — undo = unmark.
    Delete { row_idx: usize },
    /// A cell was edited — undo = revert to original (or remove edit entirely).
    Edit { row_idx: usize, col_idx: usize },
    /// A new row was added — undo = remove from new_rows.
    AddRow { new_row_idx: usize },
}

/// Accumulates pending mutations before they are committed to the database.
pub struct PendingChanges {
    /// Row indices (in the current result set) marked for deletion.
    pub deleted_rows: HashSet<usize>,
    /// Edited cells keyed by (row_idx, col_idx).
    pub edited_cells: HashMap<(usize, usize), EditedCell>,
    /// New rows to insert (appended at the bottom of the grid).
    pub new_rows: Vec<NewRow>,
    /// Undo stack — most recent action at the end.
    pub undo_stack: Vec<UndoEntry>,
}

impl PendingChanges {
    pub fn new() -> Self {
        Self {
            deleted_rows: HashSet::new(),
            edited_cells: HashMap::new(),
            new_rows: Vec::new(),
            undo_stack: Vec::new(),
        }
    }

    /// Mark a row for deletion (toggle: if already marked, unmark it).
    pub fn toggle_delete(&mut self, row_idx: usize) {
        if self.deleted_rows.contains(&row_idx) {
            self.deleted_rows.remove(&row_idx);
            // Don't push undo for un-delete; user toggled manually
        } else {
            self.deleted_rows.insert(row_idx);
            self.undo_stack.push(UndoEntry::Delete { row_idx });
        }
    }

    /// Record a cell edit.
    pub fn edit_cell(
        &mut self,
        row_idx: usize,
        col_idx: usize,
        column_name: String,
        original_value: DbValue,
        new_value: DbValue,
    ) {
        // If already edited, keep the original_value from the first edit
        let original = if let Some(existing) = self.edited_cells.get(&(row_idx, col_idx)) {
            existing.original_value.clone()
        } else {
            original_value
        };
        self.edited_cells.insert(
            (row_idx, col_idx),
            EditedCell {
                column_name,
                original_value: original,
                new_value,
            },
        );
        self.undo_stack.push(UndoEntry::Edit { row_idx, col_idx });
    }

    /// Add a new row to the pending buffer.
    pub fn add_row(&mut self, values: HashMap<String, DbValue>) {
        let idx = self.new_rows.len();
        self.new_rows.push(NewRow { values });
        self.undo_stack.push(UndoEntry::AddRow { new_row_idx: idx });
    }

    /// Pop the last action from the undo stack and revert it.
    pub fn undo(&mut self) {
        let entry = match self.undo_stack.pop() {
            Some(e) => e,
            None => return,
        };
        match entry {
            UndoEntry::Delete { row_idx } => {
                self.deleted_rows.remove(&row_idx);
            }
            UndoEntry::Edit { row_idx, col_idx } => {
                self.edited_cells.remove(&(row_idx, col_idx));
            }
            UndoEntry::AddRow { new_row_idx } => {
                if new_row_idx < self.new_rows.len() {
                    self.new_rows.remove(new_row_idx);
                    // Fix undo stack entries that reference shifted indices
                    for entry in &mut self.undo_stack {
                        if let UndoEntry::AddRow { new_row_idx: idx } = entry {
                            if *idx > new_row_idx {
                                *idx -= 1;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Whether there are any uncommitted changes.
    pub fn has_changes(&self) -> bool {
        !self.deleted_rows.is_empty() || !self.edited_cells.is_empty() || !self.new_rows.is_empty()
    }

    /// Count total pending operations.
    pub fn change_count(&self) -> usize {
        self.deleted_rows.len() + self.edited_cells.len() + self.new_rows.len()
    }

    /// Clear all pending changes (after successful execute or discard).
    pub fn clear(&mut self) {
        self.deleted_rows.clear();
        self.edited_cells.clear();
        self.new_rows.clear();
        self.undo_stack.clear();
    }

    /// Whether a specific row is marked for deletion.
    pub fn is_row_deleted(&self, row_idx: usize) -> bool {
        self.deleted_rows.contains(&row_idx)
    }

    /// Whether a specific cell has been edited.
    pub fn is_cell_edited(&self, row_idx: usize, col_idx: usize) -> bool {
        self.edited_cells.contains_key(&(row_idx, col_idx))
    }

    /// Get the edited value for a cell (if any).
    pub fn get_edited_value(&self, row_idx: usize, col_idx: usize) -> Option<&EditedCell> {
        self.edited_cells.get(&(row_idx, col_idx))
    }
}
