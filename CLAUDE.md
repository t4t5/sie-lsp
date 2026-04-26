# sie-lsp — architecture notes

A single-crate repo: language server (`sie-lsp` binary) and CLI (`sie` binary)
for the [SIE 4B](../docs/spec.md) file format. The parsing layer lives in the
sibling [`sie-parser`](../sie-parser) repo, consumed as a normal crates.io
dep — see [Local development](#local-development) below for working against
an unpublished local copy.

## Layout

```
Cargo.toml           package manifest (sie-parser pulled from crates.io)
justfile             build/test/install recipes
src/
├── lib.rs              re-exports SemanticToken / TOKEN_TYPES / semantic_tokens
├── semtok.rs           walks the parse output, emits absolute SemanticTokens
├── main.rs             `sie validate|dump` CLI (imports from sie_parser)
└── bin/sie-lsp.rs      tower-lsp Backend over stdio (imports from sie_parser + sie_lsp)
```

## Local development

`Cargo.toml` depends on the published `sie-parser` from crates.io. To iterate
against a local checkout instead, drop a gitignored `.cargo/config.toml` in
this repo with:

```toml
paths = ["../sie-parser"]
```

Cargo's [`paths` override](https://doc.rust-lang.org/cargo/reference/overriding-dependencies.html#paths-overrides)
swaps in the local copy whenever its name + semver match the registry
version, so nothing needs to change in `Cargo.toml` and `cargo publish`
keeps working. The expected sibling layout:

```
sie/
├── sie-parser/   (sibling repo — github.com/t4t5/sie-parser)
└── sie-lsp/      (this repo)
```

## Load-bearing design decisions

Each of these is more expensive to change than to keep. If you're about to
undo one, re-read the reasoning first.

### 1. LSP sees UTF-8 only
The Neovim plugin sets `fileencoding=cp437`, so the editor hands us UTF-8.
The CLI uses `sie_parser::read_file` which auto-detects. **Do not** thread
encoding detection into the LSP flow — `didOpen.text` is already decoded
by the client.

### 2. Byte-offset spans, converted at the LSP boundary
`sie-parser`'s AST uses byte offsets. The LSP `Range` type wants UTF-16
code-unit columns (actually char-count in Neovim's case). Conversion
happens in `src/bin/sie-lsp.rs::span_to_range` + the parser's
`offset_to_line_col`. Span conversion lives in this crate, not
`sie-parser`, because it's presentation-layer.

### 3. Semantic token legend is load-bearing
`src/semtok.rs::TOKEN_TYPES` is `&["keyword", "string", "number",
"enumMember", "operator", "macro"]` in that exact order. Indices are used
in the LSP five-tuple wire format. If you reorder, keep
`SemanticTokenKind::legend_index()` in sync. Don't add modifier bits unless
you also advertise them in `initialize`. Semantic tokens live here, not
in `sie-parser`, because the token kinds map directly to LSP's
presentation layer.

### 4. Stateless parse-on-every-request
Every LSP request re-parses via `sie_parser::parse`. A full parse of the
4k-line sample is sub-millisecond in release mode, so no incremental
parsing, no rope, no document tree.

## Deferred / explicitly out of scope

Don't implement these without re-checking with the user:

- **Semantic validation** (mostly belongs in `sie-parser`): verifications
  balancing to zero, `#TRANS` account existing in `#KONTO`, `#RAR` year
  ranges, `#KSUMMA` CRC-32, `#RTRANS` / `#TRANS` pairing.
- **Document symbols / outline**: easy-ish (one symbol per `#VER`,
  account groups), just not needed yet.
- **Goto definition**: `#TRANS` account → `#KONTO` declaration,
  `#OBJEKT` → `#DIM`.
- **Formatting**: field alignment is a common user request.
- **Folding**: `#VER` blocks are the obvious fold targets.

## Verification checklist

Before considering a change complete, from this repo's root:

```sh
just test         # all must pass
just build        # release build
./target/release/sie validate ../sie-parser/tests/fixtures/sample.se   # exit 0
./target/release/sie validate ../sie-parser/tests/fixtures/broken.se   # exit 1
```

For changes to parser semantics or diagnostic codes, run the sister repo's
tests too (`cd ../sie-parser && just test`).

## Install locally

```sh
cargo install --path .
```

This puts `sie` and `sie-lsp` on your `$PATH`.

## CLI usage

```sh
sie validate path/to/file.se     # exit 1 if any Error-severity diagnostics
sie dump path/to/file.se          # pretty-printed JSON of the parse tree
sie --help
```
