/// SQL generation and execution for table editor changes.
use suprim_core::db::commands::DbCommand;
use suprim_core::db::dialect::SqlDialect;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::EditableColumn;

/// Combine base type + optional param into the full SQL type string.
/// e.g. `("varchar", "255")` → `"varchar(255)"`, `("bigint", "")` → `"bigint"`.
fn full_type(col: &EditableColumn) -> String {
    if col.type_param.is_empty() {
        col.db_type.clone()
    } else {
        format!("{}({})", col.db_type, col.type_param)
    }
}

/// Generates CREATE TABLE DDL for a brand-new table.
pub fn generate_create_table_sql(
    schema_name: &str,
    table_name: &str,
    columns: &[EditableColumn],
    dialect: SqlDialect,
) -> String {
    if table_name.is_empty() {
        return "-- Table name is required".to_string();
    }
    let valid_cols: Vec<&EditableColumn> = columns.iter().filter(|c| !c.name.is_empty()).collect();
    if valid_cols.is_empty() {
        return "-- At least one column is required".to_string();
    }

    let full_table = dialect.quote_table(schema_name, table_name);
    let mut col_defs: Vec<String> = Vec::new();
    let mut pk_cols: Vec<String> = Vec::new();

    for col in &valid_cols {
        let mut def = format!("    {} {}", dialect.quote_ident(&col.name), full_type(col));
        if !col.nullable {
            def.push_str(" NOT NULL");
        }
        if !col.default_value.is_empty() {
            def.push_str(&format!(" DEFAULT {}", col.default_value));
        }
        col_defs.push(def);
        if col.is_primary_key {
            pk_cols.push(dialect.quote_ident(&col.name));
        }
    }

    if !pk_cols.is_empty() {
        col_defs.push(format!("    PRIMARY KEY ({})", pk_cols.join(", ")));
    }

    format!(
        "CREATE TABLE {} (\n{}\n);",
        full_table,
        col_defs.join(",\n")
    )
}

/// Generates ALTER TABLE ADD COLUMN statements for newly added columns only.
/// Does not handle column modifications or drops — those are out of scope for now.
pub fn generate_add_columns_sql(
    schema_name: &str,
    table_name: &str,
    columns: &[EditableColumn],
    dialect: SqlDialect,
) -> String {
    let full_table = dialect.quote_table(schema_name, table_name);
    let mut statements: Vec<String> = Vec::new();

    for col in columns {
        if !col.original && !col.name.is_empty() {
            let mut stmt = format!(
                "ALTER TABLE {} ADD COLUMN {} {}",
                full_table,
                dialect.quote_ident(&col.name),
                full_type(col)
            );
            if !col.nullable {
                stmt.push_str(" NOT NULL");
            }
            if !col.default_value.is_empty() {
                stmt.push_str(&format!(" DEFAULT {}", col.default_value));
            }
            stmt.push(';');
            statements.push(stmt);
        }
    }

    if statements.is_empty() {
        "-- No changes detected".to_string()
    } else {
        statements.join("\n")
    }
}

/// Sends generated SQL via the DbCommand channel.
/// Returns a status message string.
pub fn execute_changes(
    conn_id: Uuid,
    tab_id: Uuid,
    database: &str,
    sql: &str,
    columns: &mut [EditableColumn],
    cmd_tx: &mpsc::Sender<DbCommand>,
) -> String {
    if sql.starts_with("-- No") {
        return "No changes to apply.".to_string();
    }

    let result = cmd_tx.try_send(DbCommand::Execute {
        conn_id,
        tab_id,
        sql: sql.to_string(),
        database: Some(database.to_string()),
    });

    match result {
        Ok(_) => {
            // Mark new columns as original after submit
            for col in columns.iter_mut() {
                col.original = true;
            }
            "Changes submitted.".to_string()
        }
        Err(e) => format!("Failed to send: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use suprim_core::db::dialect::SqlDialect;

    fn sample_columns() -> Vec<EditableColumn> {
        vec![
            EditableColumn {
                name: "id".to_string(),
                db_type: "INT".to_string(),
                type_param: String::new(),
                nullable: false,
                is_primary_key: true,
                default_value: String::new(),
                original: false,
            },
            EditableColumn {
                name: "name".to_string(),
                db_type: "VARCHAR".to_string(),
                type_param: "255".to_string(),
                nullable: true,
                is_primary_key: false,
                default_value: String::new(),
                original: false,
            },
        ]
    }

    #[test]
    fn create_table_pg_uses_double_quotes() {
        let sql =
            generate_create_table_sql("public", "users", &sample_columns(), SqlDialect::Postgres);
        assert!(
            sql.contains("\"public\".\"users\""),
            "PG should use schema.table: {sql}"
        );
        assert!(
            sql.contains("\"id\""),
            "PG should quote column names: {sql}"
        );
    }

    #[test]
    fn create_table_mysql_uses_backticks() {
        let sql = generate_create_table_sql("mydb", "users", &sample_columns(), SqlDialect::Mysql);
        assert!(sql.contains("`users`"), "MySQL should use backtick: {sql}");
        assert!(sql.contains("`id`"), "MySQL should quote columns: {sql}");
        assert!(
            !sql.contains("\"users\""),
            "MySQL should NOT use double-quotes: {sql}"
        );
        // MySQL should not prefix schema
        assert!(
            !sql.contains("`mydb`."),
            "MySQL table should not have schema prefix: {sql}"
        );
    }

    #[test]
    fn create_table_empty_name_returns_comment() {
        let sql = generate_create_table_sql("public", "", &sample_columns(), SqlDialect::Postgres);
        assert!(sql.contains("-- Table name is required"));
    }

    #[test]
    fn create_table_no_columns_returns_comment() {
        let sql = generate_create_table_sql("public", "t", &[], SqlDialect::Postgres);
        assert!(sql.contains("-- At least one column is required"));
    }

    #[test]
    fn add_columns_mysql_backtick() {
        let mut cols = sample_columns();
        cols[0].original = true; // id already exists
                                 // name is new (original = false)
        let sql = generate_add_columns_sql("mydb", "users", &cols, SqlDialect::Mysql);
        assert!(
            sql.contains("ALTER TABLE `users`"),
            "MySQL ALTER should use backtick: {sql}"
        );
        assert!(
            sql.contains("ADD COLUMN `name`"),
            "MySQL ADD COLUMN should use backtick: {sql}"
        );
    }

    #[test]
    fn add_columns_no_new_columns_returns_comment() {
        let mut cols = sample_columns();
        cols[0].original = true;
        cols[1].original = true;
        let sql = generate_add_columns_sql("public", "t", &cols, SqlDialect::Postgres);
        assert!(sql.contains("-- No changes"));
    }
}
