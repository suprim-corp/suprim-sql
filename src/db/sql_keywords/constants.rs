/// SQL constants (uppercase) for syntax highlighting and autocomplete.
use std::collections::HashSet;
use std::sync::LazyLock;

pub static SQL_CONSTANTS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "TRUE",
        "FALSE",
        "NULL",
        "CURRENT_TIMESTAMP",
        "CURRENT_DATE",
        "CURRENT_TIME",
    ])
});
