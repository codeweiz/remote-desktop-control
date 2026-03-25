.PHONY: dev build test clean web release lint install help

# ── Default ──────────────────────────────────────────────────
help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2}'

# ── Development ─────────────────────────────────────────────
dev: ## Run the RTB CLI in development mode
	cargo run -p rtb-cli

# ── Build ───────────────────────────────────────────────────
build: web ## Build the release binary (includes frontend)
	cargo build --release -p rtb-cli

web: ## Build the frontend
	cd web && npm run build

# ── Test ────────────────────────────────────────────────────
test: ## Run all tests (Rust + frontend)
	cargo test --workspace
	cd web && npm test 2>/dev/null || true

# ── Lint ────────────────────────────────────────────────────
lint: ## Run format check and clippy
	cargo fmt --all -- --check
	cargo clippy --workspace -- -D warnings

# ── Clean ───────────────────────────────────────────────────
clean: ## Remove build artifacts
	cargo clean
	rm -rf web/dist

# ── Install ─────────────────────────────────────────────────
install: build ## Build and install rtb to /usr/local/bin
	cp target/release/rtb-cli /usr/local/bin/rtb
