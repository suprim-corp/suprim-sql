//! Build `mysql://` connection URL from DriverParams.

/// Build a connection URL from DriverParams::Mysql.
/// Caller provides the plaintext password (retrieved from keychain beforehand).
#[cfg(test)]
pub(super) fn build_connection_url(
    host: &str,
    port: u16,
    database: &str,
    user: &str,
    password: &str,
) -> String {
    format!(
        "mysql://{}:{}@{}:{}/{}",
        urlencoding_simple(user),
        urlencoding_simple(password),
        host,
        port,
        urlencoding_simple(database)
    )
}

/// Minimal percent-encoding for user/password/database segments.
pub fn urlencoding_simple(s: &str) -> String {
    s.chars()
        .flat_map(|c| match c {
            '%' => "%25".chars().collect::<Vec<_>>(),
            '@' => vec!['%', '4', '0'],
            ':' => vec!['%', '3', 'A'],
            '/' => vec!['%', '2', 'F'],
            '?' => vec!['%', '3', 'F'],
            '#' => vec!['%', '2', '3'],
            ' ' => vec!['%', '2', '0'],
            c => vec![c],
        })
        .collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoding_no_special_chars() {
        assert_eq!(urlencoding_simple("user"), "user");
    }

    #[test]
    fn urlencoding_at_sign() {
        assert_eq!(urlencoding_simple("user@host"), "user%40host");
    }

    #[test]
    fn urlencoding_colon() {
        assert_eq!(urlencoding_simple("p@ss:word"), "p%40ss%3Aword");
    }

    #[test]
    fn urlencoding_slash() {
        assert_eq!(urlencoding_simple("a/b"), "a%2Fb");
    }

    #[test]
    fn urlencoding_space() {
        assert_eq!(urlencoding_simple("my pass"), "my%20pass");
    }

    #[test]
    fn urlencoding_empty() {
        assert_eq!(urlencoding_simple(""), "");
    }

    #[test]
    fn urlencoding_question_mark() {
        assert_eq!(urlencoding_simple("pass?word"), "pass%3Fword");
    }

    #[test]
    fn urlencoding_hash() {
        assert_eq!(urlencoding_simple("pass#word"), "pass%23word");
    }

    #[test]
    fn urlencoding_percent() {
        assert_eq!(urlencoding_simple("100%done"), "100%25done");
    }

    #[test]
    fn build_url_basic() {
        let url = build_connection_url("localhost", 3306, "mydb", "user", "pass");
        assert_eq!(url, "mysql://user:pass@localhost:3306/mydb");
    }

    #[test]
    fn build_url_special_chars_in_password() {
        let url = build_connection_url("localhost", 3306, "mydb", "user", "p@ss:word");
        assert_eq!(url, "mysql://user:p%40ss%3Aword@localhost:3306/mydb");
    }

    #[test]
    fn build_url_custom_port() {
        let url = build_connection_url("db.example.com", 3307, "prod", "admin", "secret");
        assert_eq!(url, "mysql://admin:secret@db.example.com:3307/prod");
    }
}
