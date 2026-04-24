//! `sie-lsp` — language server for SIE 4B files. Stdio transport, tower-lsp.

use std::collections::HashMap;
use std::sync::Mutex;

use sie_lsp::{semantic_tokens, SemanticTokenKind, TOKEN_TYPES};
use sie_parser::{
    all_labels, label_info, offset_to_line_col, parse, FieldKind, Item, ParseOutput, Severity,
};
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    documents: Mutex<HashMap<Url, String>>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: Mutex::new(HashMap::new()),
        }
    }

    async fn publish_diagnostics(&self, uri: Url, text: &str) {
        let out = parse(text);
        let diags: Vec<Diagnostic> = out
            .diagnostics
            .iter()
            .map(|d| Diagnostic {
                range: span_to_range(text, d.span.byte_offset, d.span.byte_len),
                severity: Some(map_severity(d.severity)),
                code: Some(NumberOrString::String(d.code.to_string())),
                source: Some("sie".to_string()),
                message: d.message.clone(),
                ..Default::default()
            })
            .collect();
        self.client.publish_diagnostics(uri, diags, None).await;
    }
}

fn map_severity(s: Severity) -> DiagnosticSeverity {
    match s {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Info => DiagnosticSeverity::INFORMATION,
        Severity::Hint => DiagnosticSeverity::HINT,
    }
}

fn span_to_range(text: &str, byte_offset: usize, byte_len: usize) -> Range {
    let (l0, c0) = offset_to_line_col(text, byte_offset);
    let (l1, c1) = offset_to_line_col(text, byte_offset + byte_len);
    Range {
        start: Position::new(l0, c0),
        end: Position::new(l1, c1),
    }
}

/// Convert LSP `(line, character)` (character = UTF-8 scalar-value index) to
/// a byte offset in the source text. Counterpart to `offset_to_line_col`.
fn position_to_offset(text: &str, line: u32, character: u32) -> usize {
    let mut current_line = 0u32;
    let mut line_start = 0usize;
    for (i, b) in text.as_bytes().iter().enumerate() {
        if current_line == line {
            break;
        }
        if *b == b'\n' {
            current_line += 1;
            line_start = i + 1;
        }
    }
    // Walk `character` scalar values within this line.
    let rest = &text[line_start..];
    let mut chars = 0u32;
    for (byte_idx, _ch) in rest.char_indices() {
        if chars == character {
            return line_start + byte_idx;
        }
        chars += 1;
    }
    line_start + rest.len()
}

fn line_text(text: &str, line: u32) -> &str {
    let mut current_line = 0u32;
    let mut start = 0usize;
    let bytes = text.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if current_line == line {
            start = i;
            break;
        }
        if *b == b'\n' {
            current_line += 1;
        }
    }
    if current_line < line {
        return "";
    }
    // Find end of line.
    let end = bytes[start..]
        .iter()
        .position(|&b| b == b'\n' || b == b'\r')
        .map(|p| start + p)
        .unwrap_or(bytes.len());
    // Safe because the slice boundaries fall on ASCII \n / \r.
    &text[start..end]
}

fn find_label_at_offset<'a>(items: &'a [Item], offset: usize) -> Option<&'a Item> {
    for item in items {
        let s = item.label_span;
        if offset >= s.byte_offset && offset < s.end() {
            return Some(item);
        }
        if offset >= item.span.byte_offset && offset < item.span.end() {
            if let Some(child) = find_label_at_offset(&item.children, offset) {
                return Some(child);
            }
        }
    }
    None
}

fn hover_markdown(label: &str) -> Option<String> {
    let spec = label_info(label)?;
    Some(format!(
        "### {}\n\n```\n{}\n```\n\n{}",
        spec.label, spec.format, spec.description
    ))
}

fn label_completion_items() -> Vec<CompletionItem> {
    all_labels()
        .iter()
        .map(|spec| CompletionItem {
            label: spec.label.to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some(spec.format.to_string()),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: spec.description.to_string(),
            })),
            insert_text: Some(spec.label.to_string()),
            ..Default::default()
        })
        .collect()
}

/// If the line so far is a label position (empty or `#`+identifier prefix),
/// offer label completion. Otherwise, if we're inside an enum-kind field for
/// a known label, offer the variants. Otherwise, no completion.
fn completion_items_for_cursor(text: &str, line: u32, character: u32) -> Option<Vec<CompletionItem>> {
    let line_str = line_text(text, line);
    let char_idx = character as usize;
    let prefix_byte_end = line_str
        .char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(line_str.len());
    let prefix = &line_str[..prefix_byte_end];

    // Label position: only whitespace then optional `#` + ident chars.
    let trimmed = prefix.trim_start();
    if trimmed.is_empty() || trimmed == "#" || is_label_prefix(trimmed) {
        return Some(label_completion_items());
    }

    // Otherwise: walk the line to find field index under the cursor.
    // Tokenize prefix so far with a lightweight split that mirrors the parser's
    // whitespace/quote rules — exact parity isn't critical here, just good enough
    // to figure out which field we're in.
    let (label, field_index) = field_position_in_prefix(prefix)?;
    let spec = label_info(label)?;
    let fspec = spec.fields.get(field_index)?;
    match &fspec.kind {
        FieldKind::Enum(variants) => Some(
            variants
                .iter()
                .map(|v| CompletionItem {
                    label: v.to_string(),
                    kind: Some(CompletionItemKind::ENUM_MEMBER),
                    detail: Some(format!("{} — {}", fspec.name, spec.label)),
                    ..Default::default()
                })
                .collect(),
        ),
        _ => None,
    }
}

fn is_label_prefix(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.is_empty() || bytes[0] != b'#' {
        return false;
    }
    bytes[1..]
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
}

/// Walk the prefix of the current line (everything before the cursor) and
/// return `(label, field_index)` where `field_index` is the 0-based index
/// into the label's `fields` array of the field currently being typed.
fn field_position_in_prefix(prefix: &str) -> Option<(&str, usize)> {
    let trimmed_start = prefix.trim_start();
    if !trimmed_start.starts_with('#') {
        return None;
    }
    // Find the end of the label token.
    let label_end = trimmed_start
        .find(|c: char| c.is_whitespace())
        .unwrap_or(trimmed_start.len());
    let label = &trimmed_start[..label_end];

    // Count field "separators" after the label.
    // A field boundary is any run of whitespace outside a quoted string
    // or an inline {...} block. Only count fields that are complete (i.e.
    // followed by whitespace) — the partial token under the cursor is the
    // current field.
    let rest = &trimmed_start[label_end..];
    let bytes = rest.as_bytes();
    let mut i = 0;
    let mut field_index = 0usize;
    let mut in_field = false;
    let mut in_quotes = false;
    let mut brace_depth = 0i32;
    let mut escape = false;

    while i < bytes.len() {
        let b = bytes[i];
        if in_quotes {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_quotes = false;
            }
            i += 1;
            continue;
        }
        if brace_depth > 0 {
            if b == b'}' {
                brace_depth -= 1;
            } else if b == b'{' {
                brace_depth += 1;
            }
            i += 1;
            continue;
        }
        match b {
            b' ' | b'\t' => {
                if in_field {
                    in_field = false;
                    field_index += 1;
                }
            }
            b'"' => {
                in_quotes = true;
                in_field = true;
            }
            b'{' => {
                brace_depth += 1;
                in_field = true;
            }
            _ => {
                in_field = true;
            }
        }
        i += 1;
    }

    Some((label, field_index))
}

fn build_semantic_tokens(text: &str, out: &ParseOutput) -> Vec<SemanticToken> {
    let tokens = semantic_tokens(out);
    let mut result: Vec<SemanticToken> = Vec::with_capacity(tokens.len());
    let mut prev_line = 0u32;
    let mut prev_col = 0u32;
    for t in tokens {
        let (line, col) = offset_to_line_col(text, t.span.byte_offset);
        let (_end_line, end_col) = offset_to_line_col(text, t.span.byte_offset + t.span.byte_len);
        // If the span crosses a newline, clamp length to end-of-line — semantic
        // tokens are per-line in LSP. For our grammar this should be rare.
        let length = if line == _end_line {
            end_col.saturating_sub(col)
        } else {
            let line_str = line_text(text, line);
            line_str.chars().count() as u32 - col
        };
        let (dl, dc) = if result.is_empty() {
            (line, col)
        } else if line == prev_line {
            (0, col - prev_col)
        } else {
            (line - prev_line, col)
        };
        result.push(SemanticToken {
            delta_line: dl,
            delta_start: dc,
            length,
            token_type: token_type_index(t.kind),
            token_modifiers_bitset: 0,
        });
        prev_line = line;
        prev_col = col;
    }
    result
}

fn token_type_index(k: SemanticTokenKind) -> u32 {
    k.legend_index()
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["#".to_string()]),
                    ..Default::default()
                }),
                semantic_tokens_provider: Some(
                    SemanticTokensServerCapabilities::SemanticTokensOptions(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: TOKEN_TYPES
                                    .iter()
                                    .map(|s| SemanticTokenType::new(s))
                                    .collect(),
                                token_modifiers: vec![],
                            },
                            range: Some(false),
                            full: Some(SemanticTokensFullOptions::Bool(true)),
                            ..Default::default()
                        },
                    ),
                ),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "sie-lsp".to_string(),
                version: Some(env!("CARGO_PKG_VERSION").to_string()),
            }),
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "sie-lsp initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        let text = params.text_document.text;
        self.documents
            .lock()
            .unwrap()
            .insert(uri.clone(), text.clone());
        self.publish_diagnostics(uri, &text).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.clone();
        if let Some(change) = params.content_changes.into_iter().last() {
            let text = change.text;
            self.documents
                .lock()
                .unwrap()
                .insert(uri.clone(), text.clone());
            self.publish_diagnostics(uri, &text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .lock()
            .unwrap()
            .remove(&params.text_document.uri);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = &params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let text = {
            let docs = self.documents.lock().unwrap();
            match docs.get(uri) {
                Some(t) => t.clone(),
                None => return Ok(None),
            }
        };

        let offset = position_to_offset(&text, pos.line, pos.character);
        let out = parse(&text);
        let Some(item) = find_label_at_offset(&out.items, offset) else {
            return Ok(None);
        };
        let Some(md) = hover_markdown(&item.label) else {
            return Ok(None);
        };
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: md,
            }),
            range: Some(span_to_range(&text, item.label_span.byte_offset, item.label_span.byte_len)),
        }))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = &params.text_document_position.text_document.uri;
        let pos = params.text_document_position.position;

        let text = {
            let docs = self.documents.lock().unwrap();
            match docs.get(uri) {
                Some(t) => t.clone(),
                None => return Ok(None),
            }
        };

        let items = completion_items_for_cursor(&text, pos.line, pos.character);
        Ok(items.map(CompletionResponse::Array))
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> Result<Option<SemanticTokensResult>> {
        let uri = &params.text_document.uri;
        let text = {
            let docs = self.documents.lock().unwrap();
            match docs.get(uri) {
                Some(t) => t.clone(),
                None => return Ok(None),
            }
        };
        let out = parse(&text);
        let data = build_semantic_tokens(&text, &out);
        Ok(Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data,
        })))
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
