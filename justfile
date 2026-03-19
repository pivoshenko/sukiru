fmt:
    cargo fmt

lint:
    cargo clippy -- -D warnings

test:
    cargo test

build:
    cargo build --release

changelog:
    git-cliff --output CHANGELOG.md

check: fmt lint test build
