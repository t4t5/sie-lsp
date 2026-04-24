//! Stable string codes for every diagnostic the parser can emit. Kept here to
//! avoid string drift between the parser, tests, and docs.

pub const UNKNOWN_LABEL: &str = "unknown-label";
pub const MISSING_REQUIRED_FIELD: &str = "missing-required-field";
pub const BAD_DATE_FORMAT: &str = "bad-date-format";
pub const BAD_AMOUNT: &str = "bad-amount";
pub const BAD_INTEGER: &str = "bad-integer";
pub const BAD_ENUM_VALUE: &str = "bad-enum-value";
pub const UNCLOSED_QUOTE: &str = "unclosed-quote";
pub const UNCLOSED_BRACE: &str = "unclosed-brace";
pub const UNEXPECTED_CLOSE_BRACE: &str = "unexpected-close-brace";
pub const CONTROL_CHAR_IN_STRING: &str = "control-char-in-string";
pub const FLAGGA_NOT_FIRST: &str = "flagga-not-first";
pub const TRANS_OUTSIDE_VER: &str = "trans-outside-ver";
pub const ORPHAN_BRACE_BLOCK: &str = "orphan-brace-block";
pub const EXPECTED_OBJECT_LIST: &str = "expected-object-list";

pub const ALL: &[&str] = &[
    UNKNOWN_LABEL,
    MISSING_REQUIRED_FIELD,
    BAD_DATE_FORMAT,
    BAD_AMOUNT,
    BAD_INTEGER,
    BAD_ENUM_VALUE,
    UNCLOSED_QUOTE,
    UNCLOSED_BRACE,
    UNEXPECTED_CLOSE_BRACE,
    CONTROL_CHAR_IN_STRING,
    FLAGGA_NOT_FIRST,
    TRANS_OUTSIDE_VER,
    ORPHAN_BRACE_BLOCK,
    EXPECTED_OBJECT_LIST,
];
