default: build

build:
    cargo build --release --workspace

test:
    cargo test --workspace

run file:
    cargo run --bin sie -- validate {{file}}

dump file:
    cargo run --bin sie -- dump {{file}}

lsp:
    cargo run --bin sie-lsp

install:
    cargo install --path sie-lsp

fmt:
    cargo fmt --all

clippy:
    cargo clippy --workspace --all-targets -- -D warnings
