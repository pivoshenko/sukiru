format:
  cargo fmt

lint:
  cargo clippy -- -D warnings

test:
  cargo test

update:
  cargo update

build:
  cargo build --release

changelog:
  git-cliff --output CHANGELOG.md

check: format lint test build
