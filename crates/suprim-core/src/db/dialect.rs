//! SQL dialect abstraction — quoting, literal formatting, and PK heuristics
//! that vary between database engines.

use super::connection::DriverType;

/// SQL dialect — determines quoting style, literal format, and engine-specific heuristics.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SqlDialect {
    #[default]
    Postgres,
    Mysql,
    Sqlite,
}

impl SqlDialect {
    /// Quote an identifier (table/column name).
    pub fn quote_ident(&self, name: &str) -> String {
        match self {
            Self::Postgres | Self::Sqlite => format!("\"{}\"", name.replace('"', "\"\"")),
            Self::Mysql => format!("`{}`", name.replace('`', "``")),
        }
    }

    /// Quote a schema-qualified table for DDL/DML within the current database context.
    /// PG: `"schema"."table"`, MySQL/SQLite: `` `table` `` (no schema prefix).
    pub fn quote_table(&self, schema: &str, table: &str) -> String {
        match self {
            Self::Postgres => format!("{}.{}", self.quote_ident(schema), self.quote_ident(table)),
            Self::Mysql | Self::Sqlite => self.quote_ident(table),
        }
    }

    /// Quote a fully qualified table for cross-database queries.
    /// PG: `"schema"."table"`, MySQL: `` `database`.`table` ``, SQLite: `"table"`.
    pub fn quote_cross_db(&self, database: &str, table: &str) -> String {
        match self {
            Self::Postgres => format!("{}.{}", self.quote_ident(database), self.quote_ident(table)),
            Self::Mysql => format!("{}.{}", self.quote_ident(database), self.quote_ident(table)),
            Self::Sqlite => self.quote_ident(table),
        }
    }

    /// Format a JSON literal for SQL.
    pub fn json_literal(&self, raw: &str) -> String {
        let escaped = raw.replace('\'', "''");
        match self {
            Self::Postgres => format!("'{escaped}'::jsonb"),
            Self::Mysql => format!("CAST('{escaped}' AS JSON)"),
            Self::Sqlite => format!("'{escaped}'"),
        }
    }

    /// Format a bytes/binary literal for SQL.
    pub fn bytes_literal(&self, bytes: &[u8]) -> String {
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        match self {
            Self::Postgres => format!("'\\x{hex}'"),
            Self::Mysql => format!("X'{hex}'"),
            Self::Sqlite => format!("X'{hex}'"),
        }
    }

    /// PK skip heuristic for index generation — identifies the index that
    /// represents the primary key (so we can skip it in DDL output, since PK
    /// is already declared in CREATE TABLE).
    ///
    /// NOTE: This relies on naming conventions. Edge case: if a user creates a
    /// non-PK index named exactly "PRIMARY" (MySQL) or ending with "_pkey" (PG),
    /// it would be incorrectly skipped. This is extremely unlikely in practice.
    pub fn is_pk_index(&self, index_name: &str) -> bool {
        match self {
            Self::Postgres => index_name.ends_with("_pkey"),
            Self::Mysql => index_name == "PRIMARY",
            Self::Sqlite => index_name.starts_with("sqlite_autoindex_"),
        }
    }
}

impl From<DriverType> for SqlDialect {
    fn from(dt: DriverType) -> Self {
        match dt {
            DriverType::Postgres => Self::Postgres,
            DriverType::Mysql => Self::Mysql,
            DriverType::Sqlite => Self::Sqlite,
            // Fallback for drivers without SQL DDL support
            _ => Self::Postgres,
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_ident_postgres() {
        let d = SqlDialect::Postgres;
        assert_eq!(d.quote_ident("users"), "\"users\"");
        assert_eq!(d.quote_ident("my\"col"), "\"my\"\"col\"");
    }

    #[test]
    fn quote_ident_mysql() {
        let d = SqlDialect::Mysql;
        assert_eq!(d.quote_ident("users"), "`users`");
        assert_eq!(d.quote_ident("my`col"), "`my``col`");
    }

    #[test]
    fn quote_table_postgres() {
        let d = SqlDialect::Postgres;
        assert_eq!(d.quote_table("public", "users"), "\"public\".\"users\"");
    }

    #[test]
    fn quote_table_mysql() {
        let d = SqlDialect::Mysql;
        assert_eq!(d.quote_table("mydb", "users"), "`users`");
    }

    #[test]
    fn quote_cross_db_postgres() {
        let d = SqlDialect::Postgres;
        assert_eq!(d.quote_cross_db("mydb", "users"), "\"mydb\".\"users\"");
    }

    #[test]
    fn quote_cross_db_mysql() {
        let d = SqlDialect::Mysql;
        assert_eq!(d.quote_cross_db("other_db", "items"), "`other_db`.`items`");
    }

    #[test]
    fn json_literal_postgres() {
        let d = SqlDialect::Postgres;
        assert_eq!(d.json_literal(r#"{"a":1}"#), "'{\"a\":1}'::jsonb");
    }

    #[test]
    fn json_literal_mysql() {
        let d = SqlDialect::Mysql;
        assert_eq!(d.json_literal(r#"{"a":1}"#), "CAST('{\"a\":1}' AS JSON)");
    }

    #[test]
    fn bytes_literal_postgres() {
        let d = SqlDialect::Postgres;
        assert_eq!(d.bytes_literal(&[0xde, 0xad]), "'\\xdead'");
    }

    #[test]
    fn bytes_literal_mysql() {
        let d = SqlDialect::Mysql;
        assert_eq!(d.bytes_literal(&[0xde, 0xad]), "X'dead'");
    }

    #[test]
    fn is_pk_index_postgres() {
        let d = SqlDialect::Postgres;
        assert!(d.is_pk_index("users_pkey"));
        assert!(!d.is_pk_index("PRIMARY"));
    }

    #[test]
    fn is_pk_index_mysql() {
        let d = SqlDialect::Mysql;
        assert!(d.is_pk_index("PRIMARY"));
        assert!(!d.is_pk_index("users_pkey"));
    }

    #[test]
    fn from_driver_type() {
        assert_eq!(SqlDialect::from(DriverType::Postgres), SqlDialect::Postgres);
        assert_eq!(SqlDialect::from(DriverType::Mysql), SqlDialect::Mysql);
        assert_eq!(SqlDialect::from(DriverType::Sqlite), SqlDialect::Sqlite);
        // Fallback
        assert_eq!(SqlDialect::from(DriverType::Redis), SqlDialect::Postgres);
    }

    // ── Negative / edge case tests ───────────────────────────────────────

    #[test]
    fn quote_ident_injection_attempt_pg() {
        let d = SqlDialect::Postgres;
        // Double-quote in identifier → escaped by doubling
        let result = d.quote_ident(r#"table"; DROP TABLE users; --"#);
        assert_eq!(result, r#""table""; DROP TABLE users; --""#);
        // The doubled quote prevents breaking out of the identifier
        assert!(!result.starts_with("\"table\";"));
    }

    #[test]
    fn quote_ident_injection_attempt_mysql() {
        let d = SqlDialect::Mysql;
        // Backtick in identifier → escaped by doubling
        let result = d.quote_ident("table`; DROP TABLE users; --");
        assert_eq!(result, "`table``; DROP TABLE users; --`");
        assert!(!result.starts_with("`table`;"));
    }

    #[test]
    fn quote_ident_empty_string() {
        let pg = SqlDialect::Postgres;
        assert_eq!(pg.quote_ident(""), "\"\"");
        let my = SqlDialect::Mysql;
        assert_eq!(my.quote_ident(""), "``");
    }

    #[test]
    fn quote_table_empty_schema() {
        let pg = SqlDialect::Postgres;
        assert_eq!(pg.quote_table("", "users"), "\"\".\"users\"");
        let my = SqlDialect::Mysql;
        // MySQL ignores schema — just returns table
        assert_eq!(my.quote_table("", "users"), "`users`");
    }

    #[test]
    fn json_literal_with_single_quotes() {
        let pg = SqlDialect::Postgres;
        assert_eq!(
            pg.json_literal("{'key': 'val'}"),
            "'{''key'': ''val''}'::jsonb"
        );
        let my = SqlDialect::Mysql;
        assert_eq!(
            my.json_literal("{'key': 'val'}"),
            "CAST('{''key'': ''val''}' AS JSON)"
        );
    }

    #[test]
    fn bytes_literal_empty() {
        let pg = SqlDialect::Postgres;
        assert_eq!(pg.bytes_literal(&[]), "'\\x'");
        let my = SqlDialect::Mysql;
        assert_eq!(my.bytes_literal(&[]), "X''");
    }

    #[test]
    fn is_pk_index_not_fooled_by_substring() {
        let pg = SqlDialect::Postgres;
        // "_pkey" must be at the END, not just contained
        assert!(!pg.is_pk_index("pkey_something"));
        assert!(pg.is_pk_index("anything_pkey"));

        let my = SqlDialect::Mysql;
        // Must be exactly "PRIMARY", not just containing it
        assert!(!my.is_pk_index("PRIMARY_something"));
        assert!(!my.is_pk_index("not_PRIMARY"));
    }

    #[test]
    fn sqlite_dialect_uses_double_quotes() {
        let d = SqlDialect::Sqlite;
        assert_eq!(d.quote_ident("table"), "\"table\"");
        assert_eq!(d.quote_table("main", "users"), "\"users\"");
        assert_eq!(d.json_literal(r#"{"a":1}"#), "'{\"a\":1}'");
        assert_eq!(d.bytes_literal(&[0xca, 0xfe]), "X'cafe'");
        assert!(d.is_pk_index("sqlite_autoindex_users_1"));
        assert!(!d.is_pk_index("my_index"));
    }
}
