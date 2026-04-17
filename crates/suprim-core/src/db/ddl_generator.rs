//! DDL generator — reconstruct CREATE TABLE / INDEX / FK statements from schema metadata.

use super::schema::{ForeignKeyNode, IndexNode, TableNode};

/// Generate a full `CREATE TABLE` statement from a `TableNode`.
///
/// Includes columns (type, NOT NULL, DEFAULT) and a composite PRIMARY KEY
/// constraint when applicable. Does NOT include indexes or foreign keys —
/// use [`create_index_ddl`] and [`add_foreign_key_ddl`] separately.
pub fn create_table_ddl(schema: &str, tbl: &TableNode) -> String {
    let mut sql = format!("CREATE TABLE \"{schema}\".\"{}\" (\n", tbl.name);
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
            "    \"{}\" {}{null}{default}{comma}\n",
            col.name, col.db_type
        ));
    }

    if !pk_cols.is_empty() {
        let cols = pk_cols
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        sql.push_str(&format!("    PRIMARY KEY ({cols})\n"));
    }

    sql.push_str(");");
    sql
}

/// Generate a `CREATE INDEX` (or `CREATE UNIQUE INDEX`) statement.
pub fn create_index_ddl(schema: &str, table: &str, idx: &IndexNode) -> String {
    let unique = if idx.is_unique { "UNIQUE " } else { "" };
    let cols = idx
        .columns
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "CREATE {unique}INDEX \"{}\" ON \"{schema}\".\"{table}\" ({cols});",
        idx.name
    )
}

/// Generate an `ALTER TABLE … ADD CONSTRAINT … FOREIGN KEY` statement.
pub fn add_foreign_key_ddl(schema: &str, table: &str, fk: &ForeignKeyNode) -> String {
    let cols = fk
        .columns
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let ref_cols = fk
        .ref_columns
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "ALTER TABLE \"{schema}\".\"{table}\" ADD CONSTRAINT \"{}\" \
         FOREIGN KEY ({cols}) REFERENCES \"{schema}\".\"{}\" ({ref_cols});",
        fk.name, fk.ref_table
    )
}

/// Generate the full DDL for a table: CREATE TABLE + indexes + foreign keys.
pub fn full_table_ddl(schema: &str, tbl: &TableNode) -> String {
    let mut parts = vec![create_table_ddl(schema, tbl)];

    for idx in &tbl.indexes {
        // Skip indexes that are just the PK (often named <table>_pkey)
        if idx.name.ends_with("_pkey") {
            continue;
        }
        parts.push(create_index_ddl(schema, &tbl.name, idx));
    }

    for fk in &tbl.foreign_keys {
        parts.push(add_foreign_key_ddl(schema, &tbl.name, fk));
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
    fn create_table_includes_pk() {
        let ddl = create_table_ddl("public", &sample_table());
        assert!(ddl.contains("PRIMARY KEY (\"id\")"));
        assert!(ddl.contains("\"name\" varchar(255),"));
        assert!(ddl.contains("\"active\" boolean NOT NULL DEFAULT true"));
    }

    #[test]
    fn full_ddl_includes_index() {
        let ddl = full_table_ddl("public", &sample_table());
        assert!(ddl.contains("CREATE INDEX \"idx_users_name\""));
    }
}
