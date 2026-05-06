.PHONY: build run release clean check test fmt install uninstall

build:
	cargo build

run:
	cargo run -q -- $(ARGS)

release:
	cargo build --release

install:
	cargo install --locked --path cli

uninstall:
	cargo uninstall aget

clean:
	cargo clean

test:
	cargo test

check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test
	cargo build

fmt:
	cargo fmt
	cargo clippy --fix --allow-dirty --allow-staged
