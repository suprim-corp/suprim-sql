/// SQL syntax highlighter that produces an `egui::text::LayoutJob`.
///
/// Used as a custom `.layouter()` for `egui::TextEdit` — keeps full control
/// over the TextEdit widget (ID, cursor, etc.) while adding color.
use eframe::egui;
use egui::text::LayoutJob;
use egui::{Color32, FontId, TextFormat};
use std::collections::HashSet;
use std::sync::LazyLock;

// ─── SQL keyword sets ────────────────────────────────────────────────────────

static SQL_KEYWORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "ADD",
        "ALL",
        "ALTER",
        "AND",
        "ANY",
        "AS",
        "ASC",
        "BEGIN",
        "BETWEEN",
        "BY",
        "CASE",
        "CAST",
        "CHECK",
        "COLUMN",
        "COMMIT",
        "CONSTRAINT",
        "CREATE",
        "CROSS",
        "DATABASE",
        "DEFAULT",
        "DELETE",
        "DESC",
        "DISTINCT",
        "DROP",
        "ELSE",
        "END",
        "EXCEPT",
        "EXEC",
        "EXISTS",
        "FETCH",
        "FOR",
        "FOREIGN",
        "FROM",
        "FULL",
        "GRANT",
        "GROUP",
        "HAVING",
        "IF",
        "IN",
        "INDEX",
        "INNER",
        "INSERT",
        "INTERSECT",
        "INTO",
        "IS",
        "JOIN",
        "KEY",
        "LEFT",
        "LIKE",
        "ILIKE",
        "LIMIT",
        "NOT",
        "NULL",
        "OFFSET",
        "ON",
        "OR",
        "ORDER",
        "OUTER",
        "PRIMARY",
        "PROCEDURE",
        "REPLACE",
        "RETURNING",
        "REVOKE",
        "RIGHT",
        "ROLLBACK",
        "ROWNUM",
        "SELECT",
        "SET",
        "TABLE",
        "THEN",
        "TOP",
        "TRUNCATE",
        "UNION",
        "UNIQUE",
        "UPDATE",
        "VALUES",
        "VIEW",
        "WHEN",
        "WHERE",
        "WITH",
        "LATERAL",
        "RECURSIVE",
        "NATURAL",
        "USING",
        "WINDOW",
        "OVER",
        "PARTITION",
        "ROWS",
        "RANGE",
        "UNBOUNDED",
        "PRECEDING",
        "FOLLOWING",
        "CURRENT",
        "ROW",
        "FILTER",
        "WITHIN",
        "MATERIALIZED",
        "CONCURRENTLY",
        "CASCADE",
        "RESTRICT",
        "REFERENCES",
        "DEFERRABLE",
        "INITIALLY",
        "DEFERRED",
        "IMMEDIATE",
        "TRIGGER",
        "FUNCTION",
        "RETURNS",
        "LANGUAGE",
        "SCHEMA",
        "SEQUENCE",
        "OWNED",
        "NONE",
        "RENAME",
        "TO",
        "COALESCE",
        "NULLIF",
        "GREATEST",
        "LEAST",
    ])
});

static SQL_TYPES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "BOOL",
        "BOOLEAN",
        "INT",
        "INT2",
        "INT4",
        "INT8",
        "INTEGER",
        "SMALLINT",
        "BIGINT",
        "SERIAL",
        "BIGSERIAL",
        "SMALLSERIAL",
        "REAL",
        "FLOAT",
        "FLOAT4",
        "FLOAT8",
        "DOUBLE",
        "NUMERIC",
        "DECIMAL",
        "MONEY",
        "CHAR",
        "VARCHAR",
        "TEXT",
        "NAME",
        "BPCHAR",
        "BYTEA",
        "DATE",
        "TIME",
        "TIMESTAMP",
        "TIMESTAMPTZ",
        "INTERVAL",
        "UUID",
        "JSON",
        "JSONB",
        "XML",
        "ARRAY",
        "RECORD",
        "VOID",
        "OID",
        "REGCLASS",
        "REGTYPE",
        "INET",
        "CIDR",
        "MACADDR",
        "BIT",
        "VARBIT",
        "TSVECTOR",
        "TSQUERY",
        "POINT",
        "LINE",
        "LSEG",
        "BOX",
        "PATH",
        "POLYGON",
        "CIRCLE",
        "HSTORE",
    ])
});

static SQL_CONSTANTS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "TRUE",
        "FALSE",
        "NULL",
        "CURRENT_TIMESTAMP",
        "CURRENT_DATE",
        "CURRENT_TIME",
    ])
});

static SQL_FUNCTIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "COUNT",
        "SUM",
        "AVG",
        "MIN",
        "MAX",
        "ARRAY_AGG",
        "STRING_AGG",
        "COALESCE",
        "NULLIF",
        "NOW",
        "EXTRACT",
        "DATE_TRUNC",
        "TO_CHAR",
        "TO_DATE",
        "TO_TIMESTAMP",
        "TO_NUMBER",
        "UPPER",
        "LOWER",
        "TRIM",
        "LTRIM",
        "RTRIM",
        "LENGTH",
        "SUBSTRING",
        "POSITION",
        "REPLACE",
        "CONCAT",
        "LEFT",
        "RIGHT",
        "LPAD",
        "RPAD",
        "SPLIT_PART",
        "REGEXP_REPLACE",
        "REGEXP_MATCHES",
        "ABS",
        "CEIL",
        "FLOOR",
        "ROUND",
        "TRUNC",
        "MOD",
        "POWER",
        "SQRT",
        "RANDOM",
        "GENERATE_SERIES",
        "ROW_NUMBER",
        "RANK",
        "DENSE_RANK",
        "LAG",
        "LEAD",
        "FIRST_VALUE",
        "LAST_VALUE",
        "NTH_VALUE",
        "NTILE",
        "UNNEST",
        "EXISTS",
        "JSONB_BUILD_OBJECT",
        "JSON_BUILD_OBJECT",
        "JSONB_AGG",
        "JSON_AGG",
    ])
});

// ─── Color palettes ──────────────────────────────────────────────────────────

struct SqlColors {
    keyword: Color32,
    type_: Color32,
    string: Color32,
    number: Color32,
    comment: Color32,
    function: Color32,
    constant: Color32,
    punctuation: Color32,
    default: Color32,
}

fn colors_for_mode(dark: bool) -> SqlColors {
    if dark {
        SqlColors {
            keyword: Color32::from_rgb(251, 73, 52),      // Gruvbox red
            type_: Color32::from_rgb(250, 189, 47),       // Gruvbox yellow
            string: Color32::from_rgb(142, 192, 124),     // Gruvbox green
            number: Color32::from_rgb(211, 134, 155),     // Gruvbox purple
            comment: Color32::from_rgb(146, 131, 116),    // Gruvbox gray
            function: Color32::from_rgb(184, 187, 38),    // Gruvbox bright green
            constant: Color32::from_rgb(131, 165, 152),   // Gruvbox aqua
            punctuation: Color32::from_rgb(254, 128, 25), // Gruvbox orange
            default: Color32::from_rgb(235, 219, 178),    // Gruvbox fg
        }
    } else {
        SqlColors {
            keyword: Color32::from_rgb(157, 0, 6),      // Gruvbox dark red
            type_: Color32::from_rgb(181, 118, 20),     // Gruvbox dark yellow
            string: Color32::from_rgb(66, 123, 88),     // Gruvbox dark green
            number: Color32::from_rgb(143, 63, 113),    // Gruvbox dark purple
            comment: Color32::from_rgb(124, 111, 100),  // Gruvbox dark gray
            function: Color32::from_rgb(121, 116, 14),  // Gruvbox dark bright green
            constant: Color32::from_rgb(7, 102, 120),   // Gruvbox dark aqua
            punctuation: Color32::from_rgb(175, 58, 3), // Gruvbox dark orange
            default: Color32::from_rgb(40, 40, 40),     // Gruvbox dark fg
        }
    }
}

// ─── Tokenizer ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum TokenKind {
    Keyword,
    Type,
    Function,
    Constant,
    String,
    Number,
    Comment,
    Punctuation,
    Default,
}

/// Build an `egui::text::LayoutJob` with SQL syntax highlighting.
///
/// Designed to be used as `TextEdit::multiline(...).layouter(&mut layouter)`.
pub fn sql_layout_job(text: &str, font_id: FontId, dark_mode: bool) -> LayoutJob {
    let colors = colors_for_mode(dark_mode);
    let mut job = LayoutJob::default();

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // ── Single-line comment: -- ──────────────────────────────────────
        if ch == '-' && i + 1 < len && chars[i + 1] == '-' {
            let start = i;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            append_token(
                &mut job,
                text,
                start,
                i,
                &font_id,
                color_for(&colors, TokenKind::Comment),
            );
            continue;
        }

        // ── Multi-line comment: /* ... */ ────────────────────────────────
        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            let start = i;
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2; // skip */
            } else {
                i = len;
            }
            append_token(
                &mut job,
                text,
                start,
                i,
                &font_id,
                color_for(&colors, TokenKind::Comment),
            );
            continue;
        }

        // ── Strings: 'single' or "double" ───────────────────────────────
        if ch == '\'' || ch == '"' {
            let quote = ch;
            let start = i;
            i += 1;
            while i < len {
                if chars[i] == quote {
                    // Handle escaped quotes ('')
                    if i + 1 < len && chars[i + 1] == quote {
                        i += 2;
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            append_token(
                &mut job,
                text,
                start,
                i,
                &font_id,
                color_for(&colors, TokenKind::String),
            );
            continue;
        }

        // ── Dollar-quoted strings: $tag$...$tag$ ─────────────────────────
        if ch == '$' {
            let start = i;
            // Find the tag end
            let tag_start = i;
            i += 1;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            if i < len && chars[i] == '$' {
                i += 1;
                let tag: String = chars[tag_start..i].iter().collect();
                // Find closing tag
                let tag_chars: Vec<char> = tag.chars().collect();
                'outer: while i < len {
                    if chars[i] == '$' {
                        let remaining = len - i;
                        if remaining >= tag_chars.len() {
                            let candidate: Vec<char> = chars[i..i + tag_chars.len()].to_vec();
                            if candidate == tag_chars {
                                i += tag_chars.len();
                                break 'outer;
                            }
                        }
                    }
                    i += 1;
                }
                append_token(
                    &mut job,
                    text,
                    start,
                    i,
                    &font_id,
                    color_for(&colors, TokenKind::String),
                );
                continue;
            } else {
                // Not a dollar-quote, treat $ as punctuation
                i = tag_start;
                append_token(
                    &mut job,
                    text,
                    i,
                    i + 1,
                    &font_id,
                    color_for(&colors, TokenKind::Punctuation),
                );
                i += 1;
                continue;
            }
        }

        // ── Numbers ─────────────────────────────────────────────────────
        if ch.is_ascii_digit() || (ch == '.' && i + 1 < len && chars[i + 1].is_ascii_digit()) {
            let start = i;
            while i < len
                && (chars[i].is_ascii_digit()
                    || chars[i] == '.'
                    || chars[i] == 'e'
                    || chars[i] == 'E')
            {
                i += 1;
            }
            append_token(
                &mut job,
                text,
                start,
                i,
                &font_id,
                color_for(&colors, TokenKind::Number),
            );
            continue;
        }

        // ── Words (identifiers / keywords / types / functions) ──────────
        if ch.is_alphabetic() || ch == '_' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let upper = word.to_uppercase();

            // Check if followed by '(' → function
            let next_non_ws = chars[i..].iter().find(|c| !c.is_whitespace());
            let kind = if SQL_KEYWORDS.contains(upper.as_str()) {
                TokenKind::Keyword
            } else if SQL_TYPES.contains(upper.as_str()) {
                TokenKind::Type
            } else if SQL_CONSTANTS.contains(upper.as_str()) {
                TokenKind::Constant
            } else if SQL_FUNCTIONS.contains(upper.as_str()) || next_non_ws == Some(&'(') {
                TokenKind::Function
            } else {
                TokenKind::Default
            };

            append_token(&mut job, text, start, i, &font_id, color_for(&colors, kind));
            continue;
        }

        // ── Operators & punctuation ─────────────────────────────────────
        if "()[]{}.,;:=<>!+-*/%&|^~@#".contains(ch) {
            append_token(
                &mut job,
                text,
                i,
                i + 1,
                &font_id,
                color_for(&colors, TokenKind::Punctuation),
            );
            i += 1;
            continue;
        }

        // ── Whitespace & everything else ────────────────────────────────
        let start = i;
        while i < len
            && !chars[i].is_alphanumeric()
            && chars[i] != '_'
            && chars[i] != '\''
            && chars[i] != '"'
            && chars[i] != '-'
            && chars[i] != '/'
            && chars[i] != '$'
            && !"()[]{}.,;:=<>!+-*/%&|^~@#".contains(chars[i])
        {
            i += 1;
        }
        if i == start {
            i += 1; // safety: advance at least one char
        }
        append_token(
            &mut job,
            text,
            start,
            i,
            &font_id,
            color_for(&colors, TokenKind::Default),
        );
    }

    job
}

fn color_for(colors: &SqlColors, kind: TokenKind) -> Color32 {
    match kind {
        TokenKind::Keyword => colors.keyword,
        TokenKind::Type => colors.type_,
        TokenKind::Function => colors.function,
        TokenKind::Constant => colors.constant,
        TokenKind::String => colors.string,
        TokenKind::Number => colors.number,
        TokenKind::Comment => colors.comment,
        TokenKind::Punctuation => colors.punctuation,
        TokenKind::Default => colors.default,
    }
}

/// Append a highlighted token span to the layout job.
fn append_token(
    job: &mut LayoutJob,
    text: &str,
    char_start: usize,
    char_end: usize,
    font_id: &FontId,
    color: Color32,
) {
    // Convert char indices to byte indices.
    let byte_start = text
        .char_indices()
        .nth(char_start)
        .map(|(b, _)| b)
        .unwrap_or(text.len());
    let byte_end = text
        .char_indices()
        .nth(char_end)
        .map(|(b, _)| b)
        .unwrap_or(text.len());

    if byte_start < byte_end {
        job.append(
            &text[byte_start..byte_end],
            0.0,
            TextFormat::simple(font_id.clone(), color),
        );
    }
}
