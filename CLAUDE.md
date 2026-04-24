# sie-lsp — architecture notes

Cargo workspace with a language server + CLI for the [SIE 4B](../docs/spec.md) file format. Two member crates, stateless parse-on-demand.

## Layout

```
Cargo.toml           virtual workspace manifest (members: sie-parser, sie-lsp)
justfile             build/test/install recipes; run from workspace root

sie-parser/          publishable parser crate — no LSP deps
├── Cargo.toml
├── src/
│   ├── lib.rs          re-exports + read_file() + offset_to_line_col()
│   ├── types.rs        Item / Field / Span / Diagnostic
│   ├── parser.rs       line-based tokenizer + item builder + field validator
│   ├── labels.rs       LABELS: &[LabelSpec] — the schema for all 36 labels
│   ├── diagnostics.rs  stable `&'static str` codes (stringly-typed on purpose)
│   ├── cp437.rs        handrolled 128-entry CP437 high-half → Unicode table
│   └── document.rs     typed SieDocument model (Account, Company, FiscalYear, …)
└── tests/
    ├── sample.rs       parses the real 4080-line Visma export, asserts 0 errors
    ├── broken.rs       parses broken.se, asserts every diagnostic code fires
    └── fixtures/
        ├── sample.se   copy of docs/SIE4 example file.SE (CP437 bytes + CRLF preserved)
        └── broken.se   handcrafted; one occurrence of each diagnostic code

sie-lsp/             language server + `sie` CLI
├── Cargo.toml          depends on sie-parser via path
└── src/
    ├── lib.rs          re-exports SemanticToken / TOKEN_TYPES / semantic_tokens
    ├── semtok.rs       walks the parse output, emits absolute SemanticTokens
    ├── main.rs         `sie validate|dump` CLI (imports from sie_parser)
    └── bin/sie-lsp.rs  tower-lsp Backend over stdio (imports from sie_parser + sie_lsp)
```

The two crates are published separately: `sie-parser` is reusable by any SIE tool (importers, converters, analytics) without pulling in `tokio` / `tower-lsp`.

## Load-bearing design decisions

Each of these is more expensive to change than to keep. If you're about to undo one, re-read the reasoning first.

### 1. Schema-driven stringly-typed AST
`Item { label: String, fields: Vec<Field>, children: Vec<Item>, span }` with validation driven by `LABELS: &[LabelSpec]` in `sie-parser/src/labels.rs`. **Do not** replace this with 36 typed variants.
- The grammar is uniform — one label per line, whitespace-separated fields, one container (`#VER`). A schema table handles this naturally.
- Unknown labels must round-trip (spec §7.1) and unknown trailing fields must be accepted (§7.3). A typed AST needs an `Unknown` escape hatch anyway.
- `sie-parser/src/labels.rs` is the single source of truth for parsing, hover, and completion. Adding a label in a future spec rev = add a row. With typed variants it'd be a new struct + enum variant + match arm in every consumer.
- If you need richer semantic checks later (balance, cross-ref), write them as thin functions over `&[Item]` that pull fields by index. The cost is "this field name → index 2" lookups, which is fine.

### 2. Stateless `parse(input: &str) -> ParseOutput`
Every LSP request re-parses. No incremental parser, no rope, no document tree. A full parse of the 4k-line sample is sub-millisecond in release mode.
- `parse` never returns `Err` — all problems become `Diagnostic`s. The CLI and LSP both consume the same structure.
- Don't add a `Parser` struct with fields. Error recovery is per-line: emit a diagnostic, skip to next line, keep going.

### 3. Handrolled CP437 (no encoding crate)
`encoding_rs` doesn't cover CP437 (WHATWG Encoding Standard only). `codepage-437` and `oem_cp` exist but bring in a dependency for ~60 lines of code.
- `sie-parser/src/cp437.rs::CP437_HIGH` is a 128-entry `[char; 128]`. The low half is identical to ASCII so `if b < 0x80 { b as char }`.
- If you ever need more codepages (CP850, CP865), consider switching to `oem_cp`. Otherwise leave it alone.
- `detect_encoding` sniffs for `#FORMAT PC8` in the first 4 KiB, then falls back to UTF-8 validity. The LSP server itself assumes UTF-8 — encoding is handled at the file-reading / editor boundary, never in `parse()`.

### 4. LSP sees UTF-8 only
The Neovim plugin sets `fileencoding=cp437`, so the editor hands us UTF-8. The CLI uses `read_file()` which auto-detects. **Do not** thread encoding detection into `parse()` — the LSP's `didOpen.text` is already decoded by the client.

### 5. Byte-offset spans, converted at LSP boundary
Every `Span { byte_offset, byte_len }` in the AST uses byte offsets into the original source string. The LSP `Range` type wants UTF-16 code-unit columns (actually char-count in Neovim's case). Conversion happens in `sie-lsp/src/bin/sie-lsp.rs::span_to_range` + `offset_to_line_col`. Don't pre-compute line/col in the parser — it'd tie the AST to a presentation layer. This is why span conversion lives in the LSP crate, not `sie-parser`.

### 6. Quoted strings: slice, don't build byte-by-byte
`sie-parser/src/parser.rs::read_quoted` scans for the terminating `"` (respecting `\"` and `\\` escapes) and then extracts the body as a single UTF-8 `&str`, then runs `unescape_quoted_body` over it. An earlier version pushed bytes one at a time and corrupted multi-byte UTF-8 sequences (Swedish `Ö` came out as `Ã` + control char). If you touch this function, make sure the sample test still decodes `Övningsbolaget AB`.

### 7. Brace handling via 2-state machine
`ParseState { TopLevel, InsideVer { parent_index, open_span } }`. SIE only has one level of nesting (`#VER { #TRANS ... }`), so no stack is needed. If a future spec adds more containers, generalize to a stack — but for now the simple enum is more readable.

### 8. Semantic token legend is load-bearing
`sie-lsp/src/semtok.rs::TOKEN_TYPES` is `&["keyword", "string", "number", "enumMember", "operator", "macro"]` in that exact order. Indices are used in the LSP five-tuple wire format. If you reorder, keep `SemanticTokenKind::legend_index()` in sync. Don't add modifier bits unless you also advertise them in `initialize`. Semantic tokens are the one piece of parse-walking that lives in `sie-lsp`, not `sie-parser`, because the token kinds map directly to LSP's presentation layer.

### 9. Diagnostic codes are stable strings
`sie-parser/src/diagnostics.rs` defines `&'static str` codes. These are **public API** — the `broken.rs` test asserts the set, editors filter on them, external tools key off them. Don't rename without a deprecation period. Add new codes by appending to `ALL`.

### 10. Fixtures are committed intact
`sie-parser/tests/fixtures/sample.se` is a copy of the real Visma export — CP437 bytes and CRLF line endings preserved. **Do not** convert to UTF-8 or LF; the whole point is to exercise the real-file path. Editors may offer to "fix" it — decline.

### 11. Empty quoted string = absent on optional fields
In `validate_field` (`sie-parser/src/parser.rs`), an `""` token in an **optional** position short-circuits validation — no `bad-date-format` / `bad-amount` / `bad-integer` / `bad-enum-value` is emitted. Rationale: SIE fields are positional with no way to "skip" an intermediate optional field. Real exporters (notably Fortnox) write `""` as a placeholder so they can reach later positional fields like `quantity`. Example: `#TRANS 1930 {} -91.05 "" "" 0` — the two `""` placeholders stand in for `transdate` and `transtext` so `quantity=0` lands in slot 6. The spec (§11 #TRANS.1) makes those fields optional; §5.10 mandates `YYYYMMDD` for dates but is silent on the empty-placeholder case. Every SIE reader in the wild accepts it, so we do too.
- Required fields still error on `""` (e.g. `#VER`'s `verdate`, `#GEN`'s `date`). The lenience is gated on `!spec.required`.
- `FieldKind::ObjectList` is excluded — empty there is `{}`, not `""`.
- Tests: `empty_quoted_placeholder_on_optional_field` and `empty_quoted_still_errors_on_required_field` in `parser.rs`.

## Adding a new label

Spec amendments or vendor extensions:
1. Add a `LabelSpec` entry in `sie-parser/src/labels.rs` (with `description`, `format`, `fields`).
2. Update `labels::tests::has_thirty_six_labels` count.
3. Add a line to `sie-parser/tests/sample.rs` or `broken.rs` if the new label exercises a new diagnostic path.
4. No changes to `parser.rs`, `sie-lsp/src/semtok.rs`, or the LSP server should be needed — they drive off the schema.

## Adding a new diagnostic code

1. Add a `pub const FOO: &str = "foo";` to `sie-parser/src/diagnostics.rs` and append to `ALL`.
2. Emit it from wherever the check lives (usually `parser.rs::build_item` or `validate_field`).
3. Add a trigger to `sie-parser/tests/fixtures/broken.se` that hits the new code.
4. The `broken.rs` test uses `dc::ALL` so it auto-includes new codes.

## Deferred / explicitly out of scope

Don't implement these without re-checking with the user — they were deliberately excluded from the MVP to keep scope small:

- **Semantic validation**: verifications balancing to zero, `#TRANS` account existing in `#KONTO`, `#RAR` year ranges being non-overlapping, `#KSUMMA` CRC-32 verification, #RTRANS/#TRANS pairing (spec §11 #RTRANS point 4).
- **Document symbols / outline**: easy-ish (one symbol per `#VER`, account groups), just not needed yet.
- **Goto definition**: `#TRANS` account → `#KONTO` declaration, `#OBJEKT` → `#DIM`.
- **Formatting**: field alignment is a common user request.
- **Folding**: `#VER` blocks are the obvious fold targets.
- **Incremental parsing**: never needed for files this small.

## Things that have bitten me

- **rust-analyzer unlinked-file warnings** during development when a new module is written but `lib.rs` hasn't been updated yet. Ignore — real build is the source of truth.
- **Integer fields accept negative numbers** (because `#RAR` uses `year_no: 0, -1, -2`). If you add a field that must be non-negative (e.g. account numbers), consider a `FieldKind::NonNegativeInteger` variant rather than changing `FieldKind::Integer`.
- **Tab inside a quoted string is a control char** per spec §5.7. This is caught by the `b < 0x20` check. Real files never have this but the broken.se fixture does.
- **`#VER` without a brace block** is silently tolerated (the container flag is for detecting the `{` line that follows). If the next line isn't `{`, `#VER` becomes a top-level item with no children and no diagnostic. That matches real files where `#VER` can appear with its sub-entries inline on rare occasions. Don't turn this into an error without spec backing.

## Verification checklist

Before considering a change complete, from the workspace root:

```sh
just test         # all must pass
just build        # release build of both binaries
./target/release/sie validate sie-parser/tests/fixtures/sample.se   # exit 0
./target/release/sie validate sie-parser/tests/fixtures/broken.se   # exit 1
```

For LSP-path changes, the raw-stdio smoke test pattern:
```sh
# See the git log for `Initial skeleton` commit for a Python helper, or
# use any LSP client; nvim with sie.nvim is the canonical integration test.
```

## Install locally

From the workspace root:

```sh
cargo install --path sie-lsp
```

This puts `sie` and `sie-lsp` on your `$PATH`. `sie-parser` is a library-only crate — `cargo install` is a no-op on it.

## CLI usage

```sh
sie validate path/to/file.se     # exit 1 if any Error-severity diagnostics
sie dump path/to/file.se          # pretty-printed JSON of the parse tree
sie --help
```
