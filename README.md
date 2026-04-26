# SIE LSP

A language server and CLI for the [SIE file format](https://sie.se/format/) — the
Swedish standard for exchanging bookkeeping data between accounting programs.

This crate produces two binaries:

- **`sie`** — a tiny CLI that parses a `.se` / `.si` / `.sie` file and reports
  diagnostics, or emits the parsed structure as JSON.
- **`sie-lsp`** — a [Language Server Protocol](https://microsoft.github.io/language-server-protocol/)
  server providing diagnostics, hover, completion, and semantic highlighting
  in any LSP-capable editor.

The pure parsing layer (parser, CP437 codec, label schema, typed
`SieDocument` model) lives in the sibling [**`sie-parser`**](https://github.com/t4t5/sie-parser)
crate — depend on that directly if you don't need the language server.

Try the companion Neovim plugin [`sie.nvim`](https://github.com/t4t5/sie.nvim).

## Install

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

Pass `-` as the filename to read from stdin (assumed UTF-8).

## LSP capabilities

| Capability | Status |
|---|---|
| Diagnostics (syntax) | yes |
| Hover on `#LABEL` | yes |
| Completion after `#` | yes (labels + enum values) |
| Semantic tokens | yes |
| Document symbols | no (deferred) |
| Goto definition | no (deferred) |
| Balance / cross-reference validation | no (deferred) |

## Diagnostic codes

| Code | Severity | Meaning |
|---|---|---|
| `unknown-label` | Info | label starts with `#` but isn't in the SIE 4B spec (allowed per §7.1) |
| `missing-required-field` | Error | fewer fields than the label requires |
| `bad-date-format` | Error | not `YYYYMMDD`, or invalid month/day |
| `bad-amount` | Error | not `-?\d+(\.\d{1,2})?` |
| `bad-integer` | Error | non-numeric integer field |
| `bad-enum-value` | Error | value not in the allowed set (e.g. `#KTYP 1510 X`) |
| `unclosed-quote` | Error | string not terminated on the same line |
| `unclosed-brace` | Error | `#VER { ... }` block never closed before EOF |
| `unexpected-close-brace` | Error | `}` without a matching `{` |
| `control-char-in-string` | Error | ASCII 0–31 or 127 inside a quoted string (forbidden by §5.7) |
| `flagga-not-first` | Warning | first item is not `#FLAGGA` (§5.12) |
| `trans-outside-ver` | Error | `#TRANS` / `#RTRANS` / `#BTRANS` at top level |
| `orphan-brace-block` | Error | `{` with no preceding container item |
| `expected-object-list` | Error | a field expected `{ ... }` but got a bare token |

## Encoding

Real-world SIE files are encoded in **CP437** (IBM PC-8) and typically use CRLF
line endings. The CLI auto-detects the encoding (via the `#FORMAT PC8` marker
or a UTF-8 validity check) and decodes to UTF-8 internally. The LSP server
assumes the editor has already decoded the file.

## Development

```sh
just build      # cargo build --release
just test       # all unit + integration tests
just run path/to/file.se
just lsp        # run the LSP on stdio for manual debugging
```

Local development uses a `[patch.crates-io]` override in `Cargo.toml`
that points `sie-parser` at the sibling
`../sie-parser` checkout. Clone both repos as siblings:

```
sie/
├── sie-parser/
└── sie-lsp/
```

If you only have this repo cloned, comment out the `[patch.crates-io]`
section in `Cargo.toml` to fall back to the published `sie-parser` on
crates.io.
