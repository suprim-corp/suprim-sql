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

/// Generates ALTER TABLE ADD COLUMN statements for newly added columns.
pub fn generate_alter_sql(
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
