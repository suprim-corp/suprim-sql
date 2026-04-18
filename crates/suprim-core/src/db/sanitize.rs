//! Input sanitization for user-provided SQL fragments (WHERE, ORDER BY).
//!
//! The filter bar allows users to type arbitrary WHERE/ORDER BY clauses.
//! These are injected into queries wrapped in READ ONLY transactions to prevent
//! mutations. However, we add an extra layer of defense by rejecting patterns
//! that are clearly malicious or nonsensical in a filter context.
//!
//! **Threat model:** The user IS the authenticated database operator — they already
//! have full SELECT (and possibly DML) access via the SQL editor. The filter bar
//! sanitization prevents accidental multi-statement injection, not a determined
//! attacker (who can just use the SQL editor directly).

use crate::error::{AppError, Result};

/// Validate a WHERE clause fragment. Returns Ok(trimmed) or Err with reason.
pub fn validate_where_clause(clause: &str) -> Result<String> {
    let trimmed = clause.trim().to_string();
    if trimmed.is_empty() {
        return Ok(trimmed);
    }
    reject_dangerous_patterns(&trimmed, "WHERE")?;
    Ok(trimmed)
}

/// Validate an ORDER BY clause fragment. Returns Ok(trimmed) or Err with reason.
pub fn validate_order_clause(clause: &str) -> Result<String> {
    let trimmed = clause.trim().to_string();
    if trimmed.is_empty() {
        return Ok(trimmed);
    }
    reject_dangerous_patterns(&trimmed, "ORDER BY")?;
    Ok(trimmed)
}

/// Reject SQL fragments containing dangerous patterns.
fn reject_dangerous_patterns(sql: &str, context: &str) -> Result<()> {
    let upper = sql.to_uppercase();

    // Semicolons — multi-statement injection
    if sql.contains(';') {
        return Err(AppError::query(
            sql,
            format!("{context} clause must not contain semicolons"),
        ));
    }

    // Normalize whitespace (tabs, newlines, multi-spaces → single space) before keyword check
    let normalized = upper.split_whitespace().collect::<Vec<_>>().join(" ");

    // DML/DDL keywords — should never appear in WHERE/ORDER BY
    let dangerous = [
        "INSERT ",
        "UPDATE ",
        "DELETE ",
        "DROP ",
        "ALTER ",
        "CREATE ",
        "TRUNCATE ",
        "GRANT ",
        "REVOKE ",
        "EXEC ",
        "EXECUTE ",
        "CALL ",
        // Data leak via UNION or subquery — filter bar should not need these
        "UNION ",
        "UNION(",
    ];
    for kw in dangerous {
        if normalized.contains(kw) {
            return Err(AppError::query(
                sql,
                format!("{context} clause must not contain {}", kw.trim()),
            ));
        }
    }

    // Subquery — `(SELECT ...)` should not appear in filter clauses
    if normalized.contains("(SELECT ") || normalized.contains("( SELECT ") {
        return Err(AppError::query(
            sql,
            format!("{context} clause must not contain subqueries"),
        ));
    }

    // Comment markers — could hide injected code
    if sql.contains("--") || sql.contains("/*") {
        return Err(AppError::query(
            sql,
            format!("{context} clause must not contain SQL comments"),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_where_clauses() {
        assert!(validate_where_clause("age > 30").is_ok());
        assert!(validate_where_clause("name LIKE '%alice%'").is_ok());
        assert!(validate_where_clause("id IN (1, 2, 3)").is_ok());
        assert!(validate_where_clause("status = 'active' AND age >= 18").is_ok());
        assert!(validate_where_clause("  ").is_ok()); // empty = ok
    }

    #[test]
    fn valid_order_clauses() {
        assert!(validate_order_clause("name ASC").is_ok());
        assert!(validate_order_clause("age DESC, name ASC").is_ok());
        assert!(validate_order_clause("  ").is_ok());
    }

    #[test]
    fn reject_semicolons() {
        assert!(validate_where_clause("1=1; DROP TABLE users").is_err());
        assert!(validate_order_clause("name; DROP TABLE users").is_err());
    }

    #[test]
    fn reject_dml_keywords() {
        assert!(validate_where_clause("1=0 INSERT INTO users VALUES(1)").is_err());
        assert!(validate_where_clause("1=0 UPDATE users SET name='x'").is_err());
        assert!(validate_where_clause("1=0 DELETE FROM users").is_err());
        assert!(validate_where_clause("1=0 DROP TABLE users").is_err());
    }

    #[test]
    fn reject_union_and_subquery() {
        assert!(validate_where_clause("1=0 UNION SELECT * FROM mysql.user").is_err());
        assert!(validate_where_clause("1=0 UNION ALL SELECT 1").is_err());
        assert!(validate_where_clause("id IN (SELECT id FROM secrets)").is_err());
        assert!(validate_where_clause("id IN ( SELECT id FROM secrets)").is_err());
        assert!(validate_order_clause("(SELECT password FROM users LIMIT 1)").is_err());
    }

    #[test]
    fn reject_comments() {
        assert!(validate_where_clause("1=1 -- hide this").is_err());
        assert!(validate_where_clause("1=1 /* block */").is_err());
    }

    #[test]
    fn reject_ddl() {
        assert!(validate_where_clause("1=0 ALTER TABLE users ADD col INT").is_err());
        assert!(validate_where_clause("1=0 CREATE TABLE evil (id INT)").is_err());
        assert!(validate_where_clause("1=0 TRUNCATE TABLE users").is_err());
    }

    #[test]
    fn reject_keywords_with_tab_or_newline() {
        // Tab between keyword and next token should still be caught
        assert!(validate_where_clause("1=0\tINSERT\tINTO users").is_err());
        // Newline between keyword and next token should still be caught
        assert!(validate_where_clause("1=0\nINSERT\nINTO users").is_err());
        // Mixed whitespace
        assert!(validate_where_clause("1=0\n\tDROP \t TABLE users").is_err());
    }

    #[test]
    fn reject_keywords_with_multiple_spaces() {
        assert!(validate_where_clause("1=0  INSERT   INTO users").is_err());
        assert!(validate_where_clause("1=0  DELETE   FROM users").is_err());
    }
}
