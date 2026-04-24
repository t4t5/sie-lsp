//! SIE 4B language-server library.
//!
//! Parsing, encoding, labels, and the typed document model live in
//! `sie-parser`. This crate adds LSP-specific bits: semantic tokens.

pub mod semtok;

pub use semtok::{
    SemanticToken, SemanticTokenKind, TOKEN_TYPES, tokens_for as semantic_tokens,
};
