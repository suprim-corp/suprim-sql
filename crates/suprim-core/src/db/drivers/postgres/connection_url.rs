/// Build a connection URL from DriverParams::Postgres.
/// Caller provides the plaintext password (retrieved from keychain beforehand).
///
/// Test-only helper — production code uses `PgConnectOptions` builder in
/// `driver_impl.rs` which handles escaping natively.
#[cfg(test)]
pub fn build_connection_url(
    host: &str,
    port: u16,
    database: &str,
    user: &str,
    password: &str,
) -> String {
    format!(
        "postgres://{}:{}@{}:{}/{}",
        urlencoding_simple(user),
        urlencoding_simple(password),
        host,
        port,
        database
    )
}

/// Percent-encode user/password segments for Postgres connection URLs.
/// Encodes all characters that are not unreserved per RFC 3986.
///
/// Test-only — production uses `PgConnectOptions` directly.
#[cfg(test)]
pub fn urlencoding_simple(s: &str) -> String {
    use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
    utf8_percent_encode(s, NON_ALPHANUMERIC).to_string()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoding_no_special_chars() {
        // Letters are not encoded
        assert_eq!(urlencoding_simple("user"), "user");
    }

    #[test]
    fn urlencoding_at_sign() {
        assert_eq!(urlencoding_simple("user@host"), "user%40host");
    }

    #[test]
    fn urlencoding_colon() {
        assert!(urlencoding_simple("p@ss:word").contains("%40"));
        assert!(urlencoding_simple("p@ss:word").contains("%3A"));
    }

    #[test]
    fn urlencoding_slash() {
        assert!(urlencoding_simple("a/b").contains("%2F"));
    }

    #[test]
    fn urlencoding_space() {
        assert!(urlencoding_simple("my pass").contains("%20"));
    }

    #[test]
    fn urlencoding_empty() {
        assert_eq!(urlencoding_simple(""), "");
    }

    #[test]
    fn urlencoding_question_mark() {
        assert!(urlencoding_simple("pass?word").contains("%3F"));
    }

    #[test]
    fn urlencoding_hash() {
        assert!(urlencoding_simple("pass#word").contains("%23"));
    }

    #[test]
    fn urlencoding_percent_sign() {
        // Previously missed — % itself must be encoded
        assert!(urlencoding_simple("pa%ss").contains("%25"));
    }

    #[test]
    fn urlencoding_non_ascii() {
        // Previously missed — non-ASCII must be encoded
        let encoded = urlencoding_simple("pässword");
        assert!(
            !encoded.contains('ä'),
            "non-ASCII should be encoded: {encoded}"
        );
    }

    #[test]
    fn build_url_basic() {
        let url = build_connection_url("localhost", 5432, "mydb", "user", "pass");
        assert!(url.starts_with("postgres://user:pass@localhost:5432/mydb"));
    }

    #[test]
    fn build_url_special_chars_in_password() {
        let url = build_connection_url("localhost", 5432, "mydb", "user", "p@ss:word");
        assert!(url.contains("%40")); // @
        assert!(url.contains("%3A")); // :
        assert!(url.contains("localhost:5432/mydb"));
    }

    #[test]
    fn build_url_custom_port() {
        let url = build_connection_url("db.example.com", 5433, "prod", "admin", "secret");
        assert!(url.contains("db.example.com:5433/prod"));
    }
}
