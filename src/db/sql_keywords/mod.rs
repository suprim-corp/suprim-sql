//! SQL language knowledge — keyword, type, function, and constant sets.
//! Used by the syntax highlighter, autocomplete popup, and other consumers.

mod constants;
mod functions;
mod keywords;
mod types;

pub use constants::SQL_CONSTANTS;
pub use functions::SQL_FUNCTIONS;
pub use keywords::SQL_KEYWORDS;
pub use types::SQL_TYPES;
