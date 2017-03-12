# A makefile for convenience, simply calling cargo with the right parameters.

.PHONY: all release check test clean

all:
	cargo build

release:
	cargo build --release

test:
	cargo test --all

check: all test
	cargo doc
	cargo clippy

clean:
	cargo clean
