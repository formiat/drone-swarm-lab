.PHONY: build check clippy fmt fmt-check test

build: ## Build the binary
	cargo build

check: ## Type-check without producing a binary
	cargo check

clippy: ## Run clippy lints (warnings as errors)
	cargo clippy --all-targets -- -D warnings

fmt: ## Apply formatting
	cargo fmt --all

fmt-check: ## Check formatting without making changes
	cargo fmt --all --check

test: ## Run all tests
	cargo test
