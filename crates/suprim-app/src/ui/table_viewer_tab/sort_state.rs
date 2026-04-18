/// Sort state for column header click-to-sort.
/// Tracks multi-column sort with priority ordering.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// Tracks which columns are sorted and in what order.
/// First element = primary sort key.
#[derive(Debug, Clone, Default)]
pub struct SortState {
    /// Columns sorted in priority order. First = primary sort.
    pub columns: Vec<(String, SortDirection)>,
}

impl SortState {
    /// Toggle sort for a column.
    /// If not sorted → Asc. If Asc → Desc. If Desc → remove.
    /// If `multi` is false, replaces all existing sorts with just this column.
    /// If `multi` is true, appends/modifies this column preserving others.
    pub fn toggle(&mut self, column: &str, multi: bool) {
        if let Some(pos) = self.columns.iter().position(|(c, _)| c == column) {
            let dir = self.columns[pos].1;
            match dir {
                SortDirection::Asc => {
                    self.columns[pos].1 = SortDirection::Desc;
                    if !multi {
                        let entry = self.columns.remove(pos);
                        self.columns.clear();
                        self.columns.push(entry);
                    }
                }
                SortDirection::Desc => {
                    self.columns.remove(pos);
                }
            }
        } else {
            if !multi {
                self.columns.clear();
            }
            self.columns.push((column.to_string(), SortDirection::Asc));
        }
    }

    /// Build ORDER BY clause string.
    pub fn to_order_clause(&self) -> String {
        self.columns
            .iter()
            .map(|(col, dir)| {
                let d = match dir {
                    SortDirection::Asc => "ASC",
                    SortDirection::Desc => "DESC",
                };
                format!("\"{}\" {}", col, d)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Parse ORDER BY text back into state (best-effort).
    /// Handles: `"col" ASC, "col2" DESC` and `col ASC, col2 DESC` and `col, col2 desc`
    pub fn from_order_clause(clause: &str) -> Self {
        let trimmed = clause.trim();
        if trimmed.is_empty() {
            return Self::default();
        }
        let mut columns = Vec::new();
        for part in trimmed.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            // Split into tokens, handle quoted identifiers
            let tokens: Vec<&str> = part.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }
            let col_name = tokens[0].trim_matches('"').to_string();
            let dir = tokens.get(1).map(|t| t.to_uppercase()).unwrap_or_default();
            let direction = if dir == "DESC" {
                SortDirection::Desc
            } else {
                SortDirection::Asc
            };
            columns.push((col_name, direction));
        }
        Self { columns }
    }

    /// Get sort direction for a specific column.
    pub fn direction(&self, column: &str) -> Option<SortDirection> {
        self.columns
            .iter()
            .find(|(c, _)| c == column)
            .map(|(_, d)| *d)
    }

    /// Get 1-based priority index for multi-sort display.
    pub fn priority(&self, column: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|(c, _)| c == column)
            .map(|i| i + 1)
    }

    #[allow(dead_code)] // Used by tests + Phase 2/3 (filter bar integration)
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    #[allow(dead_code)] // Used by tests + Phase 2/3 (filter bar integration)
    pub fn clear(&mut self) {
        self.columns.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_single_sort_asc_desc_remove() {
        let mut s = SortState::default();
        // First click → Asc
        s.toggle("id", false);
        assert_eq!(s.direction("id"), Some(SortDirection::Asc));
        // Second click → Desc
        s.toggle("id", false);
        assert_eq!(s.direction("id"), Some(SortDirection::Desc));
        // Third click → remove
        s.toggle("id", false);
        assert!(s.is_empty());
    }

    #[test]
    fn single_sort_replaces_previous() {
        let mut s = SortState::default();
        s.toggle("id", false);
        s.toggle("name", false);
        assert_eq!(s.columns.len(), 1);
        assert_eq!(s.direction("name"), Some(SortDirection::Asc));
        assert_eq!(s.direction("id"), None);
    }

    #[test]
    fn multi_sort_appends() {
        let mut s = SortState::default();
        s.toggle("id", true);
        s.toggle("name", true);
        assert_eq!(s.columns.len(), 2);
        assert_eq!(s.priority("id"), Some(1));
        assert_eq!(s.priority("name"), Some(2));
    }

    #[test]
    fn multi_sort_toggle_existing() {
        let mut s = SortState::default();
        s.toggle("id", true);
        s.toggle("name", true);
        // Toggle id Asc → Desc in multi mode
        s.toggle("id", true);
        assert_eq!(s.direction("id"), Some(SortDirection::Desc));
        assert_eq!(s.columns.len(), 2);
    }

    #[test]
    fn to_order_clause_empty() {
        let s = SortState::default();
        assert_eq!(s.to_order_clause(), "");
    }

    #[test]
    fn to_order_clause_single() {
        let mut s = SortState::default();
        s.toggle("id", false);
        assert_eq!(s.to_order_clause(), r#""id" ASC"#);
    }

    #[test]
    fn to_order_clause_multi() {
        let mut s = SortState::default();
        s.toggle("id", true);
        s.toggle("name", true);
        s.toggle("name", true); // → Desc
        assert_eq!(s.to_order_clause(), r#""id" ASC, "name" DESC"#);
    }

    #[test]
    fn from_order_clause_roundtrip() {
        let clause = r#""id" ASC, "name" DESC"#;
        let s = SortState::from_order_clause(clause);
        assert_eq!(s.columns.len(), 2);
        assert_eq!(s.direction("id"), Some(SortDirection::Asc));
        assert_eq!(s.direction("name"), Some(SortDirection::Desc));
    }

    #[test]
    fn from_order_clause_unquoted() {
        let s = SortState::from_order_clause("id ASC, name desc");
        assert_eq!(s.direction("id"), Some(SortDirection::Asc));
        assert_eq!(s.direction("name"), Some(SortDirection::Desc));
    }

    #[test]
    fn from_order_clause_no_direction() {
        let s = SortState::from_order_clause("id, name");
        assert_eq!(s.direction("id"), Some(SortDirection::Asc));
        assert_eq!(s.direction("name"), Some(SortDirection::Asc));
    }

    #[test]
    fn from_order_clause_empty() {
        let s = SortState::from_order_clause("");
        assert!(s.is_empty());
    }
}
