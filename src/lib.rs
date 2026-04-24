//! SIE 4B language-server library.
//!
//! Parsing, encoding, labels, and the typed document model live in
//! `sie-parser`. This crate adds LSP-specific bits: semantic tokens.

pub mod semtok;

pub use semtok::{
    SemanticToken, SemanticTokenKind, TOKEN_TYPES, tokens_for as semantic_tokens,
};

// Re-export sie-parser API so existing `sie_lsp::*` import paths keep working.
pub use sie_parser::{
    cp437, diagnostics, document, labels, parser, types,
    Account, AccountNo, Company, Diagnostic, Encoding, Field, FieldKind, FieldSpec,
    FieldValue, FiscalYear, Item, LabelSpec, ParseOutput, Severity, SieDocument, Span,
    SruCode, YearIdx, all_labels, decode_cp437, detect_encoding, encode_cp437,
    label_info, offset_to_line_col, parse, read_file,
};
