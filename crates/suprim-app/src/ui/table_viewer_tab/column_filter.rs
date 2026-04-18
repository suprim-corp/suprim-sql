/// Per-column filter types and WHERE clause builder.
/// Each column can have one active filter with an operator + value(s).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOperator {
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterEqual,
    LessEqual,
    Like,
    NotLike,
    IsNull,
    IsNotNull,
    In,
    Between,
}

impl FilterOperator {
    /// Human-readable label for UI display.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Equal => "=",
            Self::NotEqual => "\u{2260}",
            Self::GreaterThan => ">",
            Self::LessThan => "<",
            Self::GreaterEqual => "\u{2265}",
            Self::LessEqual => "\u{2264}",
            Self::Like => "LIKE",
            Self::NotLike => "NOT LIKE",
            Self::IsNull => "IS NULL",
            Self::IsNotNull => "IS NOT NULL",
            Self::In => "IN",
            Self::Between => "BETWEEN",
        }
    }

    /// SQL operator string for WHERE clause generation.
    pub fn sql(&self) -> &'static str {
        match self {
            Self::Equal => "=",
            Self::NotEqual => "!=",
            Self::GreaterThan => ">",
            Self::LessThan => "<",
            Self::GreaterEqual => ">=",
            Self::LessEqual => "<=",
            Self::Like => "LIKE",
            Self::NotLike => "NOT LIKE",
            Self::IsNull => "IS NULL",
            Self::IsNotNull => "IS NOT NULL",
            Self::In => "IN",
            Self::Between => "BETWEEN",
        }
    }

    /// Whether this operator requires a value input.
    pub fn needs_value(&self) -> bool {
        !matches!(self, Self::IsNull | Self::IsNotNull)
    }

    /// Whether this operator needs a second value (BETWEEN only).
    pub fn needs_second_value(&self) -> bool {
        matches!(self, Self::Between)
    }

    /// All available operators.
    pub fn all() -> &'static [FilterOperator] {
        &[
            Self::Equal,
            Self::NotEqual,
            Self::GreaterThan,
            Self::LessThan,
            Self::GreaterEqual,
            Self::LessEqual,
            Self::Like,
            Self::NotLike,
            Self::IsNull,
            Self::IsNotNull,
            Self::In,
            Self::Between,
        ]
    }

    /// Smart default operator based on column db_type.
    pub fn default_for_type(db_type: &str) -> Self {
        let lower = db_type.to_lowercase();
        if lower.contains("int")
            || lower.contains("float")
            || lower.contains("numeric")
            || lower.contains("decimal")
            || lower.contains("serial")
            || lower.contains("bigint")
            || lower.contains("real")
            || lower.contains("double")
        {
            Self::Equal
        } else if lower.contains("varchar")
            || lower.contains("text")
            || lower.contains("char")
            || lower.contains("name")
        {
            Self::Like
        } else if lower.contains("bool") {
            Self::Equal
        } else if lower.contains("timestamp") || lower.contains("date") {
            Self::GreaterEqual
        } else {
            Self::Equal
        }
    }
}

/// A single column's filter configuration.
#[derive(Debug, Clone)]
pub struct ColumnFilter {
    pub column: String,
    pub operator: FilterOperator,
    pub value: String,
    /// Second value — used only for BETWEEN operator.
    pub value2: String,
    pub enabled: bool,
}

impl ColumnFilter {
    pub fn new(column: &str, db_type: &str) -> Self {
        Self {
            column: column.to_string(),
            operator: FilterOperator::default_for_type(db_type),
            value: String::new(),
            value2: String::new(),
            enabled: true,
        }
    }

    /// Build the WHERE fragment for this filter.
    /// Returns `None` if the filter is disabled or has an empty value for operators that need one.
    pub fn to_sql(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let col = format!("\"{}\"", self.column);
        match self.operator {
            FilterOperator::IsNull => Some(format!("{col} IS NULL")),
            FilterOperator::IsNotNull => Some(format!("{col} IS NOT NULL")),
            FilterOperator::Like => {
                if self.value.is_empty() {
                    return None;
                }
                let val = if self.value.contains('%') {
                    self.value.replace('\'', "''")
                } else {
                    format!("%{}%", self.value.replace('\'', "''"))
                };
                Some(format!("{col} LIKE '{val}'"))
            }
            FilterOperator::NotLike => {
                if self.value.is_empty() {
                    return None;
                }
                let val = if self.value.contains('%') {
                    self.value.replace('\'', "''")
                } else {
                    format!("%{}%", self.value.replace('\'', "''"))
                };
                Some(format!("{col} NOT LIKE '{val}'"))
            }
            FilterOperator::In => {
                if self.value.is_empty() {
                    return None;
                }
                // Value is comma-separated: "1, 2, 3" → IN (1, 2, 3)
                Some(format!("{col} IN ({})", self.value))
            }
            FilterOperator::Between => {
                if self.value.is_empty() || self.value2.is_empty() {
                    return None;
                }
                let v1 = self.value.replace('\'', "''");
                let v2 = self.value2.replace('\'', "''");
                Some(format!("{col} BETWEEN '{v1}' AND '{v2}'"))
            }
            _ => {
                if self.value.is_empty() {
                    return None;
                }
                let op = self.operator.sql();
                // Detect if value is numeric — if so, don't quote.
                let val = if self.value.parse::<f64>().is_ok() {
                    self.value.clone()
                } else {
                    format!("'{}'", self.value.replace('\'', "''"))
                };
                Some(format!("{col} {op} {val}"))
            }
        }
    }
}

/// Tracks all per-column filters.
#[derive(Debug, Clone, Default)]
pub struct ColumnFilterState {
    pub filters: Vec<ColumnFilter>,
}

impl ColumnFilterState {
    /// Build WHERE clause from all enabled filters, joined with AND.
    pub fn to_where_clause(&self) -> String {
        let parts: Vec<String> = self.filters.iter().filter_map(|f| f.to_sql()).collect();
        parts.join(" AND ")
    }

    /// Get filter for a specific column (if any).
    pub fn get(&self, column: &str) -> Option<&ColumnFilter> {
        self.filters.iter().find(|f| f.column == column)
    }

    /// Get mutable filter for a specific column.
    pub fn get_mut(&mut self, column: &str) -> Option<&mut ColumnFilter> {
        self.filters.iter_mut().find(|f| f.column == column)
    }

    /// Set or update filter for a column.
    pub fn set(&mut self, filter: ColumnFilter) {
        if let Some(existing) = self.get_mut(&filter.column) {
            *existing = filter;
        } else {
            self.filters.push(filter);
        }
    }

    /// Remove filter for a column.
    pub fn remove(&mut self, column: &str) {
        self.filters.retain(|f| f.column != column);
    }

    /// Clear all filters.
    pub fn clear(&mut self) {
        self.filters.clear();
    }

    /// Count of active (enabled + valid SQL) filters.
    pub fn active_count(&self) -> usize {
        self.filters.iter().filter(|f| f.to_sql().is_some()).count()
    }

    /// Whether a specific column has an active filter.
    pub fn has_filter(&self, column: &str) -> bool {
        self.get(column).and_then(|f| f.to_sql()).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_operator_numeric() {
        assert_eq!(
            FilterOperator::default_for_type("int4"),
            FilterOperator::Equal
        );
        assert_eq!(
            FilterOperator::default_for_type("bigint"),
            FilterOperator::Equal
        );
        assert_eq!(
            FilterOperator::default_for_type("float8"),
            FilterOperator::Equal
        );
        assert_eq!(
            FilterOperator::default_for_type("numeric"),
            FilterOperator::Equal
        );
    }

    #[test]
    fn default_operator_text() {
        assert_eq!(
            FilterOperator::default_for_type("varchar"),
            FilterOperator::Like
        );
        assert_eq!(
            FilterOperator::default_for_type("text"),
            FilterOperator::Like
        );
        assert_eq!(
            FilterOperator::default_for_type("name"),
            FilterOperator::Like
        );
    }

    #[test]
    fn default_operator_date() {
        assert_eq!(
            FilterOperator::default_for_type("timestamp"),
            FilterOperator::GreaterEqual
        );
        assert_eq!(
            FilterOperator::default_for_type("date"),
            FilterOperator::GreaterEqual
        );
    }

    #[test]
    fn default_operator_bool() {
        assert_eq!(
            FilterOperator::default_for_type("bool"),
            FilterOperator::Equal
        );
    }

    #[test]
    fn to_sql_equal_numeric() {
        let f = ColumnFilter {
            column: "id".into(),
            operator: FilterOperator::Equal,
            value: "42".into(),
            value2: String::new(),
            enabled: true,
        };
        assert_eq!(f.to_sql(), Some(r#""id" = 42"#.into()));
    }

    #[test]
    fn to_sql_equal_string() {
        let f = ColumnFilter {
            column: "name".into(),
            operator: FilterOperator::Equal,
            value: "alice".into(),
            value2: String::new(),
            enabled: true,
        };
        assert_eq!(f.to_sql(), Some(r#""name" = 'alice'"#.into()));
    }

    #[test]
    fn to_sql_like_auto_wrap() {
        let f = ColumnFilter {
            column: "name".into(),
            operator: FilterOperator::Like,
            value: "test".into(),
            value2: String::new(),
            enabled: true,
        };
        assert_eq!(f.to_sql(), Some(r#""name" LIKE '%test%'"#.into()));
    }

    #[test]
    fn to_sql_like_explicit_wildcard() {
        let f = ColumnFilter {
            column: "name".into(),
            operator: FilterOperator::Like,
            value: "test%".into(),
            value2: String::new(),
            enabled: true,
        };
        assert_eq!(f.to_sql(), Some(r#""name" LIKE 'test%'"#.into()));
    }

    #[test]
    fn to_sql_is_null() {
        let f = ColumnFilter {
            column: "email".into(),
            operator: FilterOperator::IsNull,
            value: String::new(),
            value2: String::new(),
            enabled: true,
        };
        assert_eq!(f.to_sql(), Some(r#""email" IS NULL"#.into()));
    }

    #[test]
    fn to_sql_between() {
        let f = ColumnFilter {
            column: "age".into(),
            operator: FilterOperator::Between,
            value: "18".into(),
            value2: "65".into(),
            enabled: true,
        };
        assert_eq!(f.to_sql(), Some(r#""age" BETWEEN '18' AND '65'"#.into()));
    }

    #[test]
    fn to_sql_between_incomplete() {
        let f = ColumnFilter {
            column: "age".into(),
            operator: FilterOperator::Between,
            value: "18".into(),
            value2: String::new(),
            enabled: true,
        };
        assert_eq!(f.to_sql(), None);
    }

    #[test]
    fn to_sql_disabled() {
        let f = ColumnFilter {
            column: "id".into(),
            operator: FilterOperator::Equal,
            value: "42".into(),
            value2: String::new(),
            enabled: false,
        };
        assert_eq!(f.to_sql(), None);
    }

    #[test]
    fn to_sql_empty_value() {
        let f = ColumnFilter {
            column: "id".into(),
            operator: FilterOperator::Equal,
            value: String::new(),
            value2: String::new(),
            enabled: true,
        };
        assert_eq!(f.to_sql(), None);
    }

    #[test]
    fn to_sql_in_operator() {
        let f = ColumnFilter {
            column: "id".into(),
            operator: FilterOperator::In,
            value: "1, 2, 3".into(),
            value2: String::new(),
            enabled: true,
        };
        assert_eq!(f.to_sql(), Some(r#""id" IN (1, 2, 3)"#.into()));
    }

    #[test]
    fn to_sql_escapes_single_quotes() {
        let f = ColumnFilter {
            column: "name".into(),
            operator: FilterOperator::Equal,
            value: "O'Brien".into(),
            value2: String::new(),
            enabled: true,
        };
        assert_eq!(f.to_sql(), Some(r#""name" = 'O''Brien'"#.into()));
    }

    #[test]
    fn state_to_where_clause_multiple() {
        let mut state = ColumnFilterState::default();
        state.set(ColumnFilter {
            column: "id".into(),
            operator: FilterOperator::GreaterThan,
            value: "10".into(),
            value2: String::new(),
            enabled: true,
        });
        state.set(ColumnFilter {
            column: "name".into(),
            operator: FilterOperator::Like,
            value: "alice".into(),
            value2: String::new(),
            enabled: true,
        });
        assert_eq!(
            state.to_where_clause(),
            r#""id" > 10 AND "name" LIKE '%alice%'"#
        );
    }

    #[test]
    fn state_active_count() {
        let mut state = ColumnFilterState::default();
        state.set(ColumnFilter {
            column: "id".into(),
            operator: FilterOperator::Equal,
            value: "1".into(),
            value2: String::new(),
            enabled: true,
        });
        state.set(ColumnFilter {
            column: "name".into(),
            operator: FilterOperator::Equal,
            value: String::new(), // invalid — no value
            value2: String::new(),
            enabled: true,
        });
        assert_eq!(state.active_count(), 1);
    }

    #[test]
    fn state_has_filter() {
        let mut state = ColumnFilterState::default();
        assert!(!state.has_filter("id"));
        state.set(ColumnFilter {
            column: "id".into(),
            operator: FilterOperator::Equal,
            value: "1".into(),
            value2: String::new(),
            enabled: true,
        });
        assert!(state.has_filter("id"));
    }

    #[test]
    fn state_remove_filter() {
        let mut state = ColumnFilterState::default();
        state.set(ColumnFilter {
            column: "id".into(),
            operator: FilterOperator::Equal,
            value: "1".into(),
            value2: String::new(),
            enabled: true,
        });
        state.remove("id");
        assert!(!state.has_filter("id"));
        assert_eq!(state.filters.len(), 0);
    }

    #[test]
    fn state_clear() {
        let mut state = ColumnFilterState::default();
        state.set(ColumnFilter::new("id", "int4"));
        state.set(ColumnFilter::new("name", "text"));
        state.clear();
        assert_eq!(state.filters.len(), 0);
    }

    #[test]
    fn state_set_updates_existing() {
        let mut state = ColumnFilterState::default();
        state.set(ColumnFilter {
            column: "id".into(),
            operator: FilterOperator::Equal,
            value: "1".into(),
            value2: String::new(),
            enabled: true,
        });
        state.set(ColumnFilter {
            column: "id".into(),
            operator: FilterOperator::GreaterThan,
            value: "5".into(),
            value2: String::new(),
            enabled: true,
        });
        assert_eq!(state.filters.len(), 1);
        assert_eq!(
            state.get("id").unwrap().operator,
            FilterOperator::GreaterThan
        );
    }
}
