//! Semantic token emission. The LSP server advertises this legend and
//! delta-encodes the tokens returned by [`tokens_for`] on the wire.

use crate::labels;
use crate::types::{FieldValue, Item, ParseOutput, SemanticToken, SemanticTokenKind};

/// The token-type legend advertised by the LSP server, in order.
/// Indices into this array are used as the `token_type` field in the
/// LSP five-tuple (`line_delta, col_delta, len, type_idx, mod_bits`).
pub const TOKEN_TYPES: &[&str] = &[
    "keyword",    // 0 — #LABEL
    "string",     // 1 — "..."
    "number",     // 2 — integers / decimals / dates (for now all the same)
    "enumMember", // 3 — T/S/K/I, PC8, AB/E/HB/…
    "operator",   // 4 — { }
    "macro",      // 5 — unknown label (distinctive color)
];

pub fn tokens_for(out: &ParseOutput) -> Vec<SemanticToken> {
    let mut toks = Vec::new();
    for item in &out.items {
        push_item(item, &mut toks);
    }
    toks.sort_by_key(|t| t.span.byte_offset);
    toks
}

fn push_item(item: &Item, out: &mut Vec<SemanticToken>) {
    let spec = labels::label_info(&item.label);
    let label_kind = if spec.is_some() {
        SemanticTokenKind::Label
    } else {
        SemanticTokenKind::Unknown
    };
    out.push(SemanticToken {
        span: item.label_span,
        kind: label_kind,
    });

    for (i, field) in item.fields.iter().enumerate() {
        let fspec = spec.and_then(|s| s.fields.get(i));
        let kind = field_kind(&field.value, fspec);
        out.push(SemanticToken {
            span: field.span,
            kind,
        });
    }

    for child in &item.children {
        push_item(child, out);
    }
}

fn field_kind(
    value: &FieldValue,
    fspec: Option<&labels::FieldSpec>,
) -> SemanticTokenKind {
    match value {
        FieldValue::Quoted { .. } => SemanticTokenKind::String,
        FieldValue::ObjectList { .. } => SemanticTokenKind::Brace,
        FieldValue::Bare { text } => match fspec.map(|f| &f.kind) {
            Some(labels::FieldKind::Integer)
            | Some(labels::FieldKind::Decimal) => SemanticTokenKind::Number,
            Some(labels::FieldKind::Date) => SemanticTokenKind::Date,
            Some(labels::FieldKind::Enum(_)) => SemanticTokenKind::Enum,
            _ => {
                // Heuristic for unknown labels: numeric-looking → Number, else String.
                if looks_numeric(text) {
                    SemanticTokenKind::Number
                } else {
                    SemanticTokenKind::String
                }
            }
        },
    }
}

fn looks_numeric(s: &str) -> bool {
    !s.is_empty()
        && s.as_bytes()
            .iter()
            .all(|&b| b.is_ascii_digit() || b == b'.' || b == b'-')
}
