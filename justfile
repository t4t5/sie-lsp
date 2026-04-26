default: build

build:
    cargo build --release

test:
    cargo test

run file:
    cargo run --bin sie -- validate {{file}}

dump file:
    cargo run --bin sie -- dump {{file}}

lsp:
    cargo run --bin sie-lsp

install:
    cargo install --path .

fmt:
    cargo fmt --all

clippy:
    cargo clippy --all-targets -- -D warnings
