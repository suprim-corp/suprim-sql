//! Default value suggestions for column editor, organized by type context.
//!
//! Returns built-in suggestions (type-aware) merged with custom schema functions.
//! Supports both PostgreSQL and MySQL default value expressions.

use suprim_core::db::dialect::SqlDialect;

/// Built-in default suggestions grouped by base type and dialect.
/// Returns suggestions relevant to the given column type.
pub fn suggestions_for_type(base_type: &str, dialect: SqlDialect) -> Vec<&'static str> {
    match dialect {
        SqlDialect::Mysql => suggestions_for_type_mysql(base_type),
        _ => suggestions_for_type_postgres(base_type),
    }
}

/// PostgreSQL default suggestions grouped by base type.
fn suggestions_for_type_postgres(base_type: &str) -> Vec<&'static str> {
    match base_type.to_lowercase().as_str() {
        // Integer types
        "bigint" | "int8" => vec!["0", "nextval('seq_name')", "gen_random_uuid()"],
        "integer" | "int" | "int4" => vec!["0", "nextval('seq_name')"],
        "smallint" | "int2" => vec!["0"],
        "serial" | "bigserial" | "smallserial" => vec![],

        // Boolean
        "boolean" | "bool" => vec!["true", "false"],

        // Text / String
        "text" | "varchar" | "character varying" | "char" | "character" | "citext" => {
            vec!["''", "NULL"]
        }

        // UUID
        "uuid" => vec!["gen_random_uuid()", "uuid_generate_v4()"],

        // Timestamps / Dates
        "timestamp"
        | "timestamp without time zone"
        | "timestamp with time zone"
        | "timestamptz" => {
            vec!["now()", "CURRENT_TIMESTAMP", "'-infinity'", "'epoch'"]
        }
        "date" => vec!["CURRENT_DATE", "now()", "'-infinity'"],
        "time" | "time without time zone" | "time with time zone" | "timetz" => {
            vec!["CURRENT_TIME", "now()", "'00:00:00'"]
        }
        "interval" => vec!["'0'", "'1 hour'", "'1 day'"],

        // Numeric / Decimal
        "numeric" | "decimal" | "real" | "float4" | "double precision" | "float8" => {
            vec!["0", "0.0"]
        }

        // JSON
        "json" | "jsonb" => vec!["'{}'", "'[]'", "'{}'::jsonb"],

        // Arrays
        t if t.ends_with("[]") => vec!["'{}'", "ARRAY[]"],

        // Network
        "inet" | "cidr" => vec!["'0.0.0.0'", "'::1'"],
        "macaddr" => vec!["'00:00:00:00:00:00'"],

        // Geometric / Range / Other
        _ => vec!["NULL"],
    }
}

/// MySQL default suggestions grouped by base type.
fn suggestions_for_type_mysql(base_type: &str) -> Vec<&'static str> {
    match base_type.to_lowercase().as_str() {
        // Integer types
        "bigint" | "int" | "integer" | "mediumint" | "smallint" | "tinyint" => {
            vec!["0", "1", "NULL"]
        }

        // Boolean (MySQL BOOL is alias for TINYINT(1))
        "boolean" | "bool" => vec!["TRUE", "FALSE", "0", "1"],

        // Text / String
        "text" | "tinytext" | "mediumtext" | "longtext" | "varchar" | "char" => {
            vec!["''", "NULL"]
        }

        // UUID (no built-in UUID type, typically CHAR(36))
        "uuid" => vec!["UUID()", "''"],

        // Timestamps / Dates
        "timestamp" | "datetime" => {
            vec!["NOW()", "CURRENT_TIMESTAMP", "NULL"]
        }
        "date" => vec!["CURRENT_DATE", "CURDATE()", "NULL"],
        "time" => vec!["CURRENT_TIME", "CURTIME()", "'00:00:00'"],
        "year" => vec!["NULL", "0"],

        // Numeric / Decimal
        "numeric" | "decimal" | "float" | "double" | "real" => vec!["0", "0.0"],

        // JSON
        "json" => vec!["'{}'", "'[]'", "NULL"],

        // Enum / Set
        "enum" | "set" => vec!["NULL"],

        // Binary
        "binary" | "varbinary" | "blob" | "tinyblob" | "mediumblob" | "longblob" => {
            vec!["NULL"]
        }

        _ => vec!["NULL"],
    }
}

/// Merge built-in suggestions with custom schema functions.
/// Filters all items by the current input text (case-insensitive prefix match).
pub fn filtered_suggestions(
    base_type: &str,
    input: &str,
    schema_functions: &[String],
    dialect: SqlDialect,
) -> Vec<String> {
    let lower_input = input.to_lowercase();

    let mut results: Vec<String> = Vec::new();

    // Built-in suggestions first
    for &s in &suggestions_for_type(base_type, dialect) {
        if lower_input.is_empty() || s.to_lowercase().contains(&lower_input) {
            results.push(s.to_string());
        }
    }

    // Schema functions (formatted as `function_name()`)
    for func in schema_functions {
        if (lower_input.is_empty() || func.to_lowercase().contains(&lower_input))
            && !results.iter().any(|r| r == func)
        {
            results.push(func.clone());
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pg_bigint_has_nextval() {
        let suggestions = suggestions_for_type("bigint", SqlDialect::Postgres);
        assert!(
            suggestions.iter().any(|s| s.contains("nextval")),
            "PG bigint should suggest nextval: {:?}",
            suggestions
        );
    }

    #[test]
    fn pg_uuid_has_gen_random() {
        let suggestions = suggestions_for_type("uuid", SqlDialect::Postgres);
        assert!(
            suggestions.iter().any(|s| s.contains("gen_random_uuid")),
            "PG uuid should suggest gen_random_uuid: {:?}",
            suggestions
        );
    }

    #[test]
    fn mysql_bigint_no_nextval() {
        let suggestions = suggestions_for_type("bigint", SqlDialect::Mysql);
        assert!(
            !suggestions.iter().any(|s| s.contains("nextval")),
            "MySQL should NOT suggest nextval: {:?}",
            suggestions
        );
    }

    #[test]
    fn mysql_timestamp_has_now() {
        let suggestions = suggestions_for_type("timestamp", SqlDialect::Mysql);
        assert!(
            suggestions.iter().any(|s| *s == "NOW()"),
            "MySQL timestamp should suggest NOW(): {:?}",
            suggestions
        );
    }

    #[test]
    fn mysql_uuid_has_uuid_function() {
        let suggestions = suggestions_for_type("uuid", SqlDialect::Mysql);
        assert!(
            suggestions.iter().any(|s| *s == "UUID()"),
            "MySQL uuid should suggest UUID(): {:?}",
            suggestions
        );
    }

    #[test]
    fn mysql_json_suggestions() {
        let suggestions = suggestions_for_type("json", SqlDialect::Mysql);
        assert!(
            suggestions.contains(&"'{}'"),
            "MySQL json should suggest '{{}}': {:?}",
            suggestions
        );
    }

    #[test]
    fn filtered_suggestions_filters_by_input() {
        let results = filtered_suggestions("timestamp", "cur", &[], SqlDialect::Mysql);
        assert!(
            results.iter().all(|s| s.to_lowercase().contains("cur")),
            "Should only contain items matching 'cur': {:?}",
            results
        );
        assert!(!results.is_empty());
    }

    #[test]
    fn filtered_suggestions_empty_input_returns_all() {
        let results = filtered_suggestions("boolean", "", &[], SqlDialect::Mysql);
        assert!(
            !results.is_empty(),
            "Empty input should return all suggestions"
        );
    }
}
