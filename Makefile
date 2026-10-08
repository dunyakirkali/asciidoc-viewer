export PATH := $(CURDIR)/target/tools/bin:$(PATH)

.PHONY: check fmt lint coverage audit tools

check: fmt lint coverage audit

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --locked --all-targets -- -D warnings

coverage:
	cargo llvm-cov --locked --no-default-features --test preview_gate --html

audit:
	cargo audit --db target/audit-db

tools:
	rustup component add clippy rustfmt llvm-tools-preview
	cargo install --locked --root target/tools --target-dir target/tools-build cargo-llvm-cov@0.9.1 cargo-audit@0.22.2
