.PHONY: build test fmt check release install

build:
	cargo build

test:
	cargo test

fmt:
	cargo fmt
	cargo clippy --fix --allow-dirty

check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test
	cargo build

release:
	cargo build --release

install:
	cargo install --path cli
