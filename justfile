fmt:
    cargo fmt

lint:
    cargo clippy -- -D warnings

test:
    cargo test

build:
    cargo build --release

check: fmt lint test build
