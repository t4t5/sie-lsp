//! Core types returned by the parser.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Span {
    pub byte_offset: usize,
    pub byte_len: usize,
}

impl Span {
    pub fn new(byte_offset: usize, byte_len: usize) -> Self {
        Self { byte_offset, byte_len }
    }
    pub fn end(&self) -> usize {
        self.byte_offset + self.byte_len
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub severity: Severity,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldValue {
    Bare { text: String },
    Quoted { text: String },
    ObjectList { tokens: Vec<String> },
}

impl FieldValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            FieldValue::Bare { text } | FieldValue::Quoted { text } => Some(text.as_str()),
            FieldValue::ObjectList { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Field {
    pub value: FieldValue,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub label: String,
    pub label_span: Span,
    pub fields: Vec<Field>,
    pub children: Vec<Item>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParseOutput {
    pub items: Vec<Item>,
    pub diagnostics: Vec<Diagnostic>,
}
