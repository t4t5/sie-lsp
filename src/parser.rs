//! SIE parser. Line-based with byte-offset tracking; recovers from per-line
//! errors without aborting. Produces an [`Item`] tree plus a list of
//! [`Diagnostic`]s per spec §5 and §7.

use crate::diagnostics as dc;
use crate::labels::{self, FieldKind, FieldSpec};
use crate::types::{
    Diagnostic, Field, FieldValue, Item, ParseOutput, Severity, Span,
};

pub fn parse(input: &str) -> ParseOutput {
    let bytes = input.as_bytes();
    let mut reader = Reader::new(bytes);
    // Strip UTF-8 BOM if present — some editors add it; spec neither forbids nor allows it.
    if reader.starts_with(&[0xEF, 0xBB, 0xBF]) {
        reader.advance(3);
    }

    let mut items: Vec<Item> = Vec::new();
    let mut diagnostics: Vec<Diagnostic> = Vec::new();
    let mut state = ParseState::TopLevel;

    while !reader.at_end() {
        reader.skip_inline_ws();
        if reader.at_end() {
            break;
        }
        let b = reader.peek_byte();
        if b == b'\n' || b == b'\r' {
            reader.consume_line_ending();
            continue;
        }

        let line_start = reader.pos;
        let tokens = tokenize_line(&mut reader, &mut diagnostics);

        if tokens.is_empty() {
            continue;
        }

        // Solo brace line?
        if tokens.len() == 1 {
            match tokens[0].kind {
                TokenKind::OpenBrace => {
                    match state {
                        ParseState::TopLevel => {
                            if matches!(
                                items.last().map(|it| it.label.as_str()),
                                Some("#VER")
                            ) {
                                state = ParseState::InsideVer {
                                    parent_index: items.len() - 1,
                                    open_span: tokens[0].span,
                                };
                            } else {
                                diagnostics.push(Diagnostic {
                                    code: dc::ORPHAN_BRACE_BLOCK,
                                    message: "`{` has no preceding container item (`#VER`)".into(),
                                    severity: Severity::Error,
                                    span: tokens[0].span,
                                });
                            }
                        }
                        ParseState::InsideVer { .. } => {
                            diagnostics.push(Diagnostic {
                                code: dc::ORPHAN_BRACE_BLOCK,
                                message: "nested brace blocks are not allowed".into(),
                                severity: Severity::Error,
                                span: tokens[0].span,
                            });
                        }
                    }
                    continue;
                }
                TokenKind::CloseBrace => {
                    match state {
                        ParseState::InsideVer { parent_index, .. } => {
                            let end = tokens[0].span.end();
                            let parent = &mut items[parent_index];
                            parent.span = Span::new(
                                parent.span.byte_offset,
                                end - parent.span.byte_offset,
                            );
                            state = ParseState::TopLevel;
                        }
                        ParseState::TopLevel => {
                            diagnostics.push(Diagnostic {
                                code: dc::UNEXPECTED_CLOSE_BRACE,
                                message: "`}` without matching `{`".into(),
                                severity: Severity::Error,
                                span: tokens[0].span,
                            });
                        }
                    }
                    continue;
                }
                _ => { /* fall through */ }
            }
        }

        // Non-solo-brace line → expect a label as the first token.
        let first = &tokens[0];
        if !matches!(first.kind, TokenKind::Bare | TokenKind::Label) {
            diagnostics.push(Diagnostic {
                code: dc::UNKNOWN_LABEL,
                message: "expected a `#LABEL` at the start of the line".into(),
                severity: Severity::Info,
                span: first.span,
            });
            continue;
        }
        let label_text = &first.text;
        if !label_text.starts_with('#') {
            diagnostics.push(Diagnostic {
                code: dc::UNKNOWN_LABEL,
                message: format!("expected `#LABEL`, got `{label_text}`"),
                severity: Severity::Info,
                span: first.span,
            });
            continue;
        }

        let item = build_item(
            &tokens,
            line_start,
            reader.pos,
            &mut diagnostics,
            matches!(state, ParseState::InsideVer { .. }),
        );

        match &mut state {
            ParseState::TopLevel => {
                items.push(item);
            }
            ParseState::InsideVer { parent_index, .. } => {
                items[*parent_index].children.push(item);
            }
        }
    }

    // Handle unclosed brace block at EOF.
    if let ParseState::InsideVer { open_span, .. } = state {
        diagnostics.push(Diagnostic {
            code: dc::UNCLOSED_BRACE,
            message: "`{` is never closed before end of file".into(),
            severity: Severity::Error,
            span: open_span,
        });
    }

    // §5.12: first item should be `#FLAGGA`.
    if let Some(first) = items.first() {
        if !first.label.eq_ignore_ascii_case("#FLAGGA") {
            diagnostics.push(Diagnostic {
                code: dc::FLAGGA_NOT_FIRST,
                message: format!(
                    "first item should be `#FLAGGA`, got `{}` (spec §5.12)",
                    first.label
                ),
                severity: Severity::Warning,
                span: first.label_span,
            });
        }
    }

    ParseOutput { items, diagnostics }
}

#[derive(Clone, Copy)]
enum ParseState {
    TopLevel,
    InsideVer {
        parent_index: usize,
        open_span: Span,
    },
}

// ---------- Reader ----------

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }
    fn peek_byte(&self) -> u8 {
        self.bytes[self.pos]
    }
    fn advance(&mut self, n: usize) {
        self.pos = (self.pos + n).min(self.bytes.len());
    }
    fn starts_with(&self, prefix: &[u8]) -> bool {
        self.bytes[self.pos..].starts_with(prefix)
    }
    fn skip_inline_ws(&mut self) {
        while self.pos < self.bytes.len() {
            match self.bytes[self.pos] {
                b' ' | b'\t' => self.pos += 1,
                _ => break,
            }
        }
    }
    fn consume_line_ending(&mut self) {
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b'\r' {
            self.pos += 1;
        }
        if self.pos < self.bytes.len() && self.bytes[self.pos] == b'\n' {
            self.pos += 1;
        }
    }
}

// ---------- Tokens ----------

#[derive(Clone, Copy, PartialEq, Eq)]
enum TokenKind {
    Label,
    Bare,
    Quoted,
    ObjectList,
    OpenBrace,
    CloseBrace,
}

struct Token {
    kind: TokenKind,
    text: String,
    object_tokens: Vec<String>,
    span: Span,
}

impl Token {
    fn empty(kind: TokenKind, span: Span) -> Self {
        Self {
            kind,
            text: String::new(),
            object_tokens: Vec::new(),
            span,
        }
    }
}

/// Tokenize exactly one logical line. Consumes the line terminator.
fn tokenize_line(reader: &mut Reader, diags: &mut Vec<Diagnostic>) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    loop {
        reader.skip_inline_ws();
        if reader.at_end() {
            break;
        }
        let b = reader.peek_byte();
        if b == b'\n' || b == b'\r' {
            reader.consume_line_ending();
            break;
        }
        let start = reader.pos;
        match b {
            b'"' => tokens.push(read_quoted(reader, diags)),
            b'{' => tokens.push(read_open_or_object_list(reader, diags)),
            b'}' => {
                reader.advance(1);
                tokens.push(Token::empty(TokenKind::CloseBrace, Span::new(start, 1)));
            }
            _ => {
                // Bare token. If it's the first token on the line and starts with '#',
                // flag as Label so downstream code can tell quickly.
                let t = read_bare(reader);
                let kind = if tokens.is_empty() && t.text.starts_with('#') {
                    TokenKind::Label
                } else {
                    TokenKind::Bare
                };
                tokens.push(Token { kind, ..t });
            }
        }
    }
    tokens
}

fn read_bare(reader: &mut Reader) -> Token {
    let start = reader.pos;
    while reader.pos < reader.bytes.len() {
        let b = reader.bytes[reader.pos];
        if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' || b == b'{' || b == b'}' || b == b'"'
        {
            break;
        }
        reader.pos += 1;
    }
    let end = reader.pos;
    let text = std::str::from_utf8(&reader.bytes[start..end])
        .unwrap_or("")
        .to_string();
    Token {
        kind: TokenKind::Bare,
        text,
        object_tokens: Vec::new(),
        span: Span::new(start, end - start),
    }
}

fn read_quoted(reader: &mut Reader, diags: &mut Vec<Diagnostic>) -> Token {
    let start = reader.pos;
    reader.advance(1); // opening "
    let body_start = reader.pos;

    let mut closed = false;
    let mut control_char_at: Option<usize> = None;
    let mut saw_unterminated = false;

    while reader.pos < reader.bytes.len() {
        let b = reader.bytes[reader.pos];
        if b == b'\\' {
            // Scan past the escape pair. \" and \\ are the meaningful escapes
            // per spec §5.7; other escapes are treated as "drop the backslash,
            // keep the next char" which round-trips safely.
            if reader.pos + 1 >= reader.bytes.len() {
                saw_unterminated = true;
                break;
            }
            let next = reader.bytes[reader.pos + 1];
            if next == b'\n' || next == b'\r' {
                saw_unterminated = true;
                break;
            }
            reader.pos += 2;
            continue;
        }
        if b == b'"' {
            closed = true;
            break;
        }
        if b == b'\n' || b == b'\r' {
            saw_unterminated = true;
            break;
        }
        if b < 0x20 || b == 0x7F {
            if control_char_at.is_none() {
                control_char_at = Some(reader.pos);
            }
        }
        reader.pos += 1;
    }

    let body_end = reader.pos; // position of the closing " or of the stop byte
    if closed {
        reader.pos += 1; // consume closing "
    }

    if saw_unterminated {
        diags.push(Diagnostic {
            code: dc::UNCLOSED_QUOTE,
            message: "unterminated string before end of line".into(),
            severity: Severity::Error,
            span: Span::new(start, reader.pos - start),
        });
    }
    if let Some(ctrl_pos) = control_char_at {
        let b = reader.bytes[ctrl_pos];
        diags.push(Diagnostic {
            code: dc::CONTROL_CHAR_IN_STRING,
            message: format!("control character 0x{b:02X} is not allowed in a string"),
            severity: Severity::Error,
            span: Span::new(ctrl_pos, 1),
        });
    }

    // The body slice is valid UTF-8 because the enclosing input is UTF-8 and
    // we only broke on ASCII bytes (`"`, `\`, `\n`, `\r`, control chars).
    let body = std::str::from_utf8(&reader.bytes[body_start..body_end]).unwrap_or("");
    let text = unescape_quoted_body(body);

    Token {
        kind: TokenKind::Quoted,
        text,
        object_tokens: Vec::new(),
        span: Span::new(start, reader.pos - start),
    }
}

fn unescape_quoted_body(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(next) if next == '"' || next == '\\' => out.push(next),
                Some(next) => {
                    // Unknown escape: keep the character literally, drop the backslash.
                    out.push(next);
                }
                None => { /* trailing backslash, already flagged */ }
            }
            continue;
        }
        out.push(c);
    }
    out
}

fn read_open_or_object_list(reader: &mut Reader, diags: &mut Vec<Diagnostic>) -> Token {
    let start = reader.pos;
    reader.advance(1); // consume {
    let saved = reader.pos;

    let mut inner_tokens = Vec::new();
    let mut found_close = false;
    let mut close_end = saved;

    loop {
        reader.skip_inline_ws();
        if reader.at_end() {
            break;
        }
        let b = reader.peek_byte();
        if b == b'\n' || b == b'\r' {
            break;
        }
        if b == b'}' {
            close_end = reader.pos + 1;
            reader.advance(1);
            found_close = true;
            break;
        }
        if b == b'"' {
            let t = read_quoted(reader, diags);
            inner_tokens.push(t.text);
        } else if b == b'{' {
            // Nested {}: not valid; emit diagnostic and bail out.
            diags.push(Diagnostic {
                code: dc::EXPECTED_OBJECT_LIST,
                message: "nested `{` in object list is not allowed".into(),
                severity: Severity::Error,
                span: Span::new(reader.pos, 1),
            });
            reader.advance(1);
        } else {
            let t = read_bare(reader);
            inner_tokens.push(t.text);
        }
    }

    if found_close {
        Token {
            kind: TokenKind::ObjectList,
            text: String::new(),
            object_tokens: inner_tokens,
            span: Span::new(start, close_end - start),
        }
    } else {
        // No matching `}` on this line → treat as a bare OpenBrace.
        // Rewind so the rest of the line can still be processed.
        reader.pos = saved;
        Token::empty(TokenKind::OpenBrace, Span::new(start, 1))
    }
}

// ---------- Item building & field validation ----------

fn build_item(
    tokens: &[Token],
    line_start: usize,
    line_end: usize,
    diagnostics: &mut Vec<Diagnostic>,
    inside_ver: bool,
) -> Item {
    let head = &tokens[0];
    let label = head.text.clone();
    let label_span = head.span;

    // Recognize labels in the table; emit Info on unknown labels (spec §7.1).
    let spec = labels::label_info(&label);
    if spec.is_none() {
        diagnostics.push(Diagnostic {
            code: dc::UNKNOWN_LABEL,
            message: format!("`{label}` is not in the SIE 4B specification"),
            severity: Severity::Info,
            span: label_span,
        });
    }

    // Check #TRANS / #RTRANS / #BTRANS placement.
    if matches!(label.as_str(), "#TRANS" | "#RTRANS" | "#BTRANS") && !inside_ver {
        diagnostics.push(Diagnostic {
            code: dc::TRANS_OUTSIDE_VER,
            message: format!("`{label}` is only allowed inside a `#VER {{ ... }}` block"),
            severity: Severity::Error,
            span: label_span,
        });
    }

    let field_tokens = &tokens[1..];

    // Arity check against the schema.
    if let Some(spec) = spec {
        let required = spec.fields.iter().filter(|f| f.required).count();
        if field_tokens.len() < required {
            diagnostics.push(Diagnostic {
                code: dc::MISSING_REQUIRED_FIELD,
                message: format!(
                    "`{}` expects {} required field(s), got {} — layout: {}",
                    spec.label,
                    required,
                    field_tokens.len(),
                    spec.format
                ),
                severity: Severity::Error,
                span: label_span,
            });
        }
    }

    // Validate each known field position.
    let mut fields = Vec::with_capacity(field_tokens.len());
    for (i, tok) in field_tokens.iter().enumerate() {
        if let Some(spec) = spec {
            if let Some(fspec) = spec.fields.get(i) {
                validate_field(tok, fspec, diagnostics);
            }
            // i >= spec.fields.len(): trailing unknown field — allowed per §7.3, no diagnostic
        }
        fields.push(token_to_field(tok));
    }

    Item {
        label,
        label_span,
        fields,
        children: Vec::new(),
        span: Span::new(line_start, line_end - line_start),
    }
}

fn token_to_field(t: &Token) -> Field {
    let value = match t.kind {
        TokenKind::Quoted => FieldValue::Quoted { text: t.text.clone() },
        TokenKind::ObjectList => FieldValue::ObjectList {
            tokens: t.object_tokens.clone(),
        },
        _ => FieldValue::Bare { text: t.text.clone() },
    };
    Field {
        value,
        span: t.span,
    }
}

fn validate_field(tok: &Token, spec: &FieldSpec, diags: &mut Vec<Diagnostic>) {
    match &spec.kind {
        FieldKind::String | FieldKind::Raw => { /* always ok */ }
        FieldKind::Integer => {
            if matches!(tok.kind, TokenKind::ObjectList | TokenKind::OpenBrace | TokenKind::CloseBrace)
                || !is_integer(&tok.text)
            {
                diags.push(Diagnostic {
                    code: dc::BAD_INTEGER,
                    message: format!(
                        "`{}` expected an integer, got `{}`",
                        spec.name,
                        display_token(tok)
                    ),
                    severity: Severity::Error,
                    span: tok.span,
                });
            }
        }
        FieldKind::Decimal => {
            if matches!(tok.kind, TokenKind::ObjectList | TokenKind::OpenBrace | TokenKind::CloseBrace)
                || !is_decimal(&tok.text)
            {
                diags.push(Diagnostic {
                    code: dc::BAD_AMOUNT,
                    message: format!(
                        "`{}` expected a decimal amount, got `{}`",
                        spec.name,
                        display_token(tok)
                    ),
                    severity: Severity::Error,
                    span: tok.span,
                });
            }
        }
        FieldKind::Date => {
            if matches!(tok.kind, TokenKind::ObjectList | TokenKind::OpenBrace | TokenKind::CloseBrace)
                || !is_date(&tok.text)
            {
                diags.push(Diagnostic {
                    code: dc::BAD_DATE_FORMAT,
                    message: format!(
                        "`{}` expected YYYYMMDD, got `{}`",
                        spec.name,
                        display_token(tok)
                    ),
                    severity: Severity::Error,
                    span: tok.span,
                });
            }
        }
        FieldKind::Enum(variants) => {
            let ok = tok.text.as_str() != ""
                && variants
                    .iter()
                    .any(|v| v.eq_ignore_ascii_case(&tok.text));
            if !ok || matches!(tok.kind, TokenKind::ObjectList) {
                diags.push(Diagnostic {
                    code: dc::BAD_ENUM_VALUE,
                    message: format!(
                        "`{}` expected one of {:?}, got `{}`",
                        spec.name,
                        variants,
                        display_token(tok)
                    ),
                    severity: Severity::Error,
                    span: tok.span,
                });
            }
        }
        FieldKind::ObjectList => {
            if !matches!(tok.kind, TokenKind::ObjectList) {
                diags.push(Diagnostic {
                    code: dc::EXPECTED_OBJECT_LIST,
                    message: format!(
                        "`{}` expected an object list `{{ ... }}`, got `{}`",
                        spec.name,
                        display_token(tok)
                    ),
                    severity: Severity::Error,
                    span: tok.span,
                });
            }
        }
    }
}

fn display_token(t: &Token) -> String {
    match t.kind {
        TokenKind::ObjectList => "{...}".to_string(),
        TokenKind::OpenBrace => "{".to_string(),
        TokenKind::CloseBrace => "}".to_string(),
        _ => t.text.clone(),
    }
}

fn is_integer(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let start = if bytes[0] == b'-' { 1 } else { 0 };
    if start == bytes.len() {
        return false;
    }
    bytes[start..].iter().all(|&b| b.is_ascii_digit())
}

fn is_decimal(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let start = if bytes[0] == b'-' { 1 } else { 0 };
    if start == bytes.len() {
        return false;
    }
    let mut saw_dot = false;
    let mut digits_after_dot = 0usize;
    let mut digits_before_dot = 0usize;
    for &b in &bytes[start..] {
        match b {
            b'0'..=b'9' => {
                if saw_dot {
                    digits_after_dot += 1;
                } else {
                    digits_before_dot += 1;
                }
            }
            b'.' if !saw_dot => {
                saw_dot = true;
            }
            _ => return false,
        }
    }
    if digits_before_dot == 0 {
        return false;
    }
    if saw_dot && (digits_after_dot == 0 || digits_after_dot > 2) {
        return false;
    }
    true
}

fn is_date(s: &str) -> bool {
    if s.len() != 8 || !s.as_bytes().iter().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let month: u32 = s[4..6].parse().unwrap();
    let day: u32 = s[6..8].parse().unwrap();
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(input: &str) -> ParseOutput {
        parse(input)
    }

    fn errs(out: &ParseOutput) -> Vec<&str> {
        out.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .map(|d| d.code)
            .collect()
    }

    #[test]
    fn basic_konto() {
        let out = parse_ok("#FLAGGA 0\n#KONTO 1510 \"Accounts receivable\"\n");
        assert!(errs(&out).is_empty(), "{:#?}", out.diagnostics);
        assert_eq!(out.items.len(), 2);
        assert_eq!(out.items[1].label, "#KONTO");
        assert_eq!(out.items[1].fields.len(), 2);
        if let FieldValue::Bare { text } = &out.items[1].fields[0].value {
            assert_eq!(text, "1510");
        } else {
            panic!("expected Bare");
        }
        if let FieldValue::Quoted { text } = &out.items[1].fields[1].value {
            assert_eq!(text, "Accounts receivable");
        } else {
            panic!("expected Quoted");
        }
    }

    #[test]
    fn escaped_quotes() {
        let out = parse_ok("#FLAGGA 0\n#KONTO 1915 \"Kassa \\\"special\\\"\"\n");
        assert!(errs(&out).is_empty());
        if let FieldValue::Quoted { text } = &out.items[1].fields[1].value {
            assert_eq!(text, "Kassa \"special\"");
        } else {
            panic!();
        }
    }

    #[test]
    fn ver_block_with_trans() {
        let src = "#FLAGGA 0\n#VER A 1 20210101 \"Test\"\n{\n#TRANS 1910 {} -1000.00\n#TRANS 3010 {} 1000.00\n}\n";
        let out = parse_ok(src);
        assert!(errs(&out).is_empty(), "{:#?}", out.diagnostics);
        assert_eq!(out.items.len(), 2);
        assert_eq!(out.items[1].label, "#VER");
        assert_eq!(out.items[1].children.len(), 2);
        assert_eq!(out.items[1].children[0].label, "#TRANS");
    }

    #[test]
    fn inline_object_list_empty() {
        let out = parse_ok(
            "#FLAGGA 0\n#VER A 1 20210101\n{\n#TRANS 1910 {} -1000.00\n}\n",
        );
        assert!(errs(&out).is_empty(), "{:#?}", out.diagnostics);
        let trans = &out.items[1].children[0];
        if let FieldValue::ObjectList { tokens } = &trans.fields[1].value {
            assert!(tokens.is_empty());
        } else {
            panic!("expected ObjectList");
        }
    }

    #[test]
    fn inline_object_list_with_pairs() {
        let out = parse_ok(
            "#FLAGGA 0\n#VER A 1 20210101\n{\n#TRANS 7010 {\"1\" \"456\" \"7\" \"47\"} 13200.00\n}\n",
        );
        assert!(errs(&out).is_empty(), "{:#?}", out.diagnostics);
        let trans = &out.items[1].children[0];
        if let FieldValue::ObjectList { tokens } = &trans.fields[1].value {
            assert_eq!(tokens, &vec!["1".to_string(), "456".to_string(), "7".to_string(), "47".to_string()]);
        } else {
            panic!();
        }
    }

    #[test]
    fn crlf_handling() {
        let lf = parse_ok("#FLAGGA 0\n#KONTO 1510 \"Cash\"\n");
        let crlf = parse_ok("#FLAGGA 0\r\n#KONTO 1510 \"Cash\"\r\n");
        assert_eq!(lf.items.len(), crlf.items.len());
        assert_eq!(errs(&lf), errs(&crlf));
    }

    #[test]
    fn error_recovery() {
        let src = "#FLAGGA 0\n#KONTO abc \"bad\"\n#KONTO 1510 \"good\"\n";
        let out = parse_ok(src);
        let codes = errs(&out);
        assert!(codes.contains(&dc::BAD_INTEGER));
        // Parser recovered: still saw three items including the last good one.
        assert_eq!(out.items.len(), 3);
    }

    #[test]
    fn byte_span_of_label() {
        let src = "#FLAGGA 0\n#KONTO 1510 \"Cash\"\n";
        let out = parse_ok(src);
        let konto = &out.items[1];
        let recovered = &src[konto.label_span.byte_offset..konto.label_span.end()];
        assert_eq!(recovered, "#KONTO");
    }

    #[test]
    fn unclosed_brace_at_eof() {
        let src = "#FLAGGA 0\n#VER A 1 20210101\n{\n#TRANS 1910 {} -1000.00\n";
        let out = parse_ok(src);
        assert!(errs(&out).contains(&dc::UNCLOSED_BRACE));
    }

    #[test]
    fn orphan_brace_block() {
        let src = "#FLAGGA 0\n{\n#TRANS 1910 {} 0.00\n}\n";
        let out = parse_ok(src);
        assert!(errs(&out).contains(&dc::ORPHAN_BRACE_BLOCK));
    }

    #[test]
    fn unexpected_close_brace() {
        let src = "#FLAGGA 0\n}\n";
        let out = parse_ok(src);
        assert!(errs(&out).contains(&dc::UNEXPECTED_CLOSE_BRACE));
    }

    #[test]
    fn trans_outside_ver() {
        let src = "#FLAGGA 0\n#TRANS 1910 {} 100.00\n";
        let out = parse_ok(src);
        assert!(errs(&out).contains(&dc::TRANS_OUTSIDE_VER));
    }

    #[test]
    fn flagga_not_first_is_warning() {
        let src = "#KONTO 1510 \"Cash\"\n";
        let out = parse_ok(src);
        let warns: Vec<&str> = out.diagnostics.iter()
            .filter(|d| d.severity == Severity::Warning)
            .map(|d| d.code).collect();
        assert!(warns.contains(&dc::FLAGGA_NOT_FIRST));
    }

    #[test]
    fn unknown_label_is_info() {
        let src = "#FLAGGA 0\n#FUTURE_ITEM 123\n";
        let out = parse_ok(src);
        let infos: Vec<&str> = out.diagnostics.iter()
            .filter(|d| d.severity == Severity::Info)
            .map(|d| d.code).collect();
        assert!(infos.contains(&dc::UNKNOWN_LABEL));
        // unknown labels do not produce errors
        assert!(errs(&out).is_empty());
    }

    #[test]
    fn unclosed_quote() {
        let src = "#FLAGGA 0\n#KONTO 1510 \"unterminated\n";
        let out = parse_ok(src);
        assert!(errs(&out).contains(&dc::UNCLOSED_QUOTE));
    }

    #[test]
    fn control_char_in_string() {
        let src = "#FLAGGA 0\n#KONTO 1510 \"bad\x01ctrl\"\n";
        let out = parse_ok(src);
        assert!(errs(&out).contains(&dc::CONTROL_CHAR_IN_STRING));
    }

    #[test]
    fn bad_date() {
        let src = "#FLAGGA 0\n#GEN 2021-01-01 AN\n";
        let out = parse_ok(src);
        assert!(errs(&out).contains(&dc::BAD_DATE_FORMAT));
    }

    #[test]
    fn bad_enum() {
        let src = "#FLAGGA 0\n#KONTO 1510 \"Cash\"\n#KTYP 1510 X\n";
        let out = parse_ok(src);
        assert!(errs(&out).contains(&dc::BAD_ENUM_VALUE));
    }

    #[test]
    fn expected_object_list() {
        let src = "#FLAGGA 0\n#VER A 1 20210101\n{\n#TRANS 1910 notabrace -1000.00\n}\n";
        let out = parse_ok(src);
        assert!(errs(&out).contains(&dc::EXPECTED_OBJECT_LIST));
    }

    #[test]
    fn missing_required_field() {
        let src = "#FLAGGA 0\n#KONTO 1510\n"; // missing account_name
        let out = parse_ok(src);
        assert!(errs(&out).contains(&dc::MISSING_REQUIRED_FIELD));
    }

    #[test]
    fn is_decimal_accepts_typical_amounts() {
        assert!(is_decimal("1234"));
        assert!(is_decimal("-1234"));
        assert!(is_decimal("1234.5"));
        assert!(is_decimal("1234.50"));
        assert!(is_decimal("-1234.50"));
        assert!(!is_decimal("1234.567"));
        assert!(!is_decimal("1,234"));
        assert!(!is_decimal("+1234"));
        assert!(!is_decimal(""));
        assert!(!is_decimal("."));
    }

    #[test]
    fn is_date_checks_month_day() {
        assert!(is_date("20210131"));
        assert!(is_date("20211231"));
        assert!(!is_date("20211301")); // month 13
        assert!(!is_date("20210132")); // day 32
        assert!(!is_date("2021-01-01"));
        assert!(!is_date("202101"));
    }
}
