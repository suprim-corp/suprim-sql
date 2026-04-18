//! DDL generator — reconstruct CREATE TABLE / INDEX / FK statements from schema metadata.

use super::dialect::SqlDialect;
use super::schema::{ForeignKeyNode, IndexNode, TableNode};

/// Generate a full `CREATE TABLE` statement from a `TableNode`.
///
/// Includes columns (type, NOT NULL, DEFAULT) and a composite PRIMARY KEY
/// constraint when applicable. Does NOT include indexes or foreign keys —
/// use [`create_index_ddl`] and [`add_foreign_key_ddl`] separately.
pub fn create_table_ddl(schema: &str, tbl: &TableNode, dialect: SqlDialect) -> String {
    let table_ref = dialect.quote_table(schema, &tbl.name);
    let mut sql = format!("CREATE TABLE {table_ref} (\n");
    let pk_cols: Vec<&str> = tbl
        .columns
        .iter()
        .filter(|c| c.is_primary_key)
        .map(|c| c.name.as_str())
        .collect();

    for (i, col) in tbl.columns.iter().enumerate() {
        let comma = if i + 1 < tbl.columns.len() || !pk_cols.is_empty() {
            ","
        } else {
            ""
        };
        let null = if col.nullable { "" } else { " NOT NULL" };
        let default = col
            .default_value
            .as_ref()
            .map(|d| format!(" DEFAULT {d}"))
            .unwrap_or_default();
        sql.push_str(&format!(
            "    {} {}{null}{default}{comma}\n",
            dialect.quote_ident(&col.name),
            col.db_type
        ));
    }

    if !pk_cols.is_empty() {
        let cols = pk_cols
            .iter()
            .map(|c| dialect.quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!("    PRIMARY KEY ({cols})\n"));
    }

    sql.push_str(");");
    sql
}

/// Generate a `CREATE INDEX` (or `CREATE UNIQUE INDEX`) statement.
pub fn create_index_ddl(schema: &str, table: &str, idx: &IndexNode, dialect: SqlDialect) -> String {
    let unique = if idx.is_unique { "UNIQUE " } else { "" };
    let cols = idx
        .columns
        .iter()
        .map(|c| dialect.quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let table_ref = dialect.quote_table(schema, table);
    let idx_name = dialect.quote_ident(&idx.name);
    format!("CREATE {unique}INDEX {idx_name} ON {table_ref} ({cols});")
}

/// Generate an `ALTER TABLE … ADD CONSTRAINT … FOREIGN KEY` statement.
pub fn add_foreign_key_ddl(
    schema: &str,
    table: &str,
    fk: &ForeignKeyNode,
    dialect: SqlDialect,
) -> String {
    let cols = fk
        .columns
        .iter()
        .map(|c| dialect.quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let ref_cols = fk
        .ref_columns
        .iter()
        .map(|c| dialect.quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let table_ref = dialect.quote_table(schema, table);
    let fk_name = dialect.quote_ident(&fk.name);
    let ref_table = dialect.quote_table(schema, &fk.ref_table);
    format!(
        "ALTER TABLE {table_ref} ADD CONSTRAINT {fk_name} \
         FOREIGN KEY ({cols}) REFERENCES {ref_table} ({ref_cols});"
    )
}

/// Generate the full DDL for a table: CREATE TABLE + indexes + foreign keys.
pub fn full_table_ddl(schema: &str, tbl: &TableNode, dialect: SqlDialect) -> String {
    let mut parts = vec![create_table_ddl(schema, tbl, dialect)];

    for idx in &tbl.indexes {
        // Skip indexes that represent the primary key
        if dialect.is_pk_index(&idx.name) {
            continue;
        }
        parts.push(create_index_ddl(schema, &tbl.name, idx, dialect));
    }

    for fk in &tbl.foreign_keys {
        parts.push(add_foreign_key_ddl(schema, &tbl.name, fk, dialect));
    }

    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::ColumnNode;
    use uuid::Uuid;

    fn sample_table() -> TableNode {
        TableNode {
            id: Uuid::new_v4(),
            name: "users".to_string(),
            columns: vec![
                ColumnNode {
                    id: Uuid::new_v4(),
                    name: "id".to_string(),
                    db_type: "bigint".to_string(),
                    nullable: false,
                    is_primary_key: true,
                    default_value: None,
                },
                ColumnNode {
                    id: Uuid::new_v4(),
                    name: "name".to_string(),
                    db_type: "varchar(255)".to_string(),
                    nullable: true,
                    is_primary_key: false,
                    default_value: None,
                },
                ColumnNode {
                    id: Uuid::new_v4(),
                    name: "active".to_string(),
                    db_type: "boolean".to_string(),
                    nullable: false,
                    is_primary_key: false,
                    default_value: Some("true".to_string()),
                },
            ],
            indexes: vec![IndexNode {
                id: Uuid::new_v4(),
                name: "idx_users_name".to_string(),
                columns: vec!["name".to_string()],
                is_unique: false,
            }],
            foreign_keys: vec![],
            row_count: Some(100),
        }
    }

    #[test]
    fn create_table_postgres_includes_pk() {
        let ddl = create_table_ddl("public", &sample_table(), SqlDialect::Postgres);
        assert!(ddl.contains("PRIMARY KEY (\"id\")"));
        assert!(ddl.contains("\"name\" varchar(255),"));
        assert!(ddl.contains("\"active\" boolean NOT NULL DEFAULT true"));
        assert!(ddl.starts_with("CREATE TABLE \"public\".\"users\""));
    }

    #[test]
    fn create_table_mysql_uses_backticks() {
        let ddl = create_table_ddl("mydb", &sample_table(), SqlDialect::Mysql);
        assert!(ddl.contains("PRIMARY KEY (`id`)"));
        assert!(ddl.contains("`name` varchar(255),"));
        assert!(ddl.contains("`active` boolean NOT NULL DEFAULT true"));
        // MySQL: no schema prefix
        assert!(ddl.starts_with("CREATE TABLE `users`"));
    }

    #[test]
    fn full_ddl_includes_index_postgres() {
        let ddl = full_table_ddl("public", &sample_table(), SqlDialect::Postgres);
        assert!(ddl.contains("CREATE INDEX \"idx_users_name\""));
    }

    #[test]
    fn full_ddl_includes_index_mysql() {
        let ddl = full_table_ddl("mydb", &sample_table(), SqlDialect::Mysql);
        assert!(ddl.contains("CREATE INDEX `idx_users_name`"));
    }

    #[test]
    fn full_ddl_skips_pk_index_postgres() {
        let mut tbl = sample_table();
        tbl.indexes.push(IndexNode {
            id: Uuid::new_v4(),
            name: "users_pkey".to_string(),
            columns: vec!["id".to_string()],
            is_unique: true,
        });
        let ddl = full_table_ddl("public", &tbl, SqlDialect::Postgres);
        assert!(!ddl.contains("users_pkey"));
    }

    #[test]
    fn full_ddl_skips_pk_index_mysql() {
        let mut tbl = sample_table();
        tbl.indexes.push(IndexNode {
            id: Uuid::new_v4(),
            name: "PRIMARY".to_string(),
            columns: vec!["id".to_string()],
            is_unique: true,
        });
        let ddl = full_table_ddl("mydb", &tbl, SqlDialect::Mysql);
        // The PK index named "PRIMARY" should be skipped (not emitted as CREATE INDEX)
        assert!(!ddl.contains("CREATE UNIQUE INDEX `PRIMARY`"));
        assert!(!ddl.contains("CREATE INDEX `PRIMARY`"));
        // But PRIMARY KEY constraint should still be in the CREATE TABLE body
        assert!(ddl.contains("PRIMARY KEY"));
    }
}
