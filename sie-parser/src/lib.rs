//! SIE 4B parser, encoder, and typed document model.
//!
//! The parser is stateless: every public entry point takes the full source as
//! input. This mirrors the `csvx` design — re-parsing on demand keeps the
//! internal model trivial and fast enough for multi-thousand line files.

pub mod cp437;
pub mod diagnostics;
pub mod document;
pub mod labels;
pub mod parser;
pub mod types;

pub use cp437::{decode_cp437, detect_encoding, encode_cp437};
pub use document::{
    Account, AccountNo, Company, FiscalYear, SieDocument, SruCode, YearIdx,
};
pub use labels::{FieldKind, FieldSpec, LabelSpec, all_labels, label_info};
pub use parser::parse;
pub use types::{
    Diagnostic, Field, FieldValue, Item, ParseOutput, Severity, Span,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Cp437,
}

/// Read a file and return a UTF-8 `String`, decoding CP437 bytes if needed.
/// A leading UTF-8 BOM is stripped silently.
pub fn read_file(path: &std::path::Path) -> std::io::Result<String> {
    let bytes = std::fs::read(path)?;
    let stripped: &[u8] = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &bytes[3..]
    } else {
        &bytes
    };
    let s = match detect_encoding(stripped) {
        Encoding::Utf8 => String::from_utf8_lossy(stripped).into_owned(),
        Encoding::Cp437 => decode_cp437(stripped),
    };
    Ok(s)
}

/// Convert a byte offset into `input` to an `(line, column)` pair.
/// Columns are counted in **UTF-8 scalar values** starting at 0 — suitable
/// for the `u16` column-count path in LSP but NOT for the UTF-16 code-unit
/// path. Callers that need UTF-16 columns must do their own fixup.
pub fn offset_to_line_col(input: &str, offset: usize) -> (u32, u32) {
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (i, b) in input.as_bytes().iter().enumerate() {
        if i == offset {
            break;
        }
        if *b == b'\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    let col_bytes = offset.saturating_sub(line_start).min(input.len() - line_start.min(input.len()));
    let line_slice = &input.as_bytes()[line_start..(line_start + col_bytes).min(input.len())];
    let col = std::str::from_utf8(line_slice)
        .map(|s| s.chars().count() as u32)
        .unwrap_or(col_bytes as u32);
    (line, col)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_mapping_basic() {
        let s = "abc\ndef\n";
        assert_eq!(offset_to_line_col(s, 0), (0, 0));
        assert_eq!(offset_to_line_col(s, 2), (0, 2));
        assert_eq!(offset_to_line_col(s, 4), (1, 0));
        assert_eq!(offset_to_line_col(s, 6), (1, 2));
    }

    #[test]
    fn offset_mapping_utf8() {
        // "Övning" — Ö is 2 bytes in UTF-8. Column count is chars, not bytes.
        let s = "Övning";
        assert_eq!(offset_to_line_col(s, 0), (0, 0));
        // After the Ö (2 bytes), column should be 1.
        assert_eq!(offset_to_line_col(s, 2), (0, 1));
    }
}
