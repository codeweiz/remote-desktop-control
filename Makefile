.PHONY: dev build test clean web install plugins install-plugins desktop help

# ── Default ──────────────────────────────────────────────────
help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2}'

# ── Core ─────────────────────────────────────────────────────
dev: install-plugins ## Start RTB in development mode (with plugins)
	cargo run -p rtb-cli -- start

build: web plugins ## Build release binary + frontend + plugins
	cargo build --release -p rtb-cli

install: build install-plugins ## Build and install rtb + plugins to system
	cp target/release/rtb-cli /usr/local/bin/rtb

# ── Frontend ─────────────────────────────────────────────────
web: ## Build the frontend
	cd web && npm run build

# ── Plugins ──────────────────────────────────────────────────
plugins: ## Build all plugins (feishu, cloudflare-tunnel)
	cargo build --manifest-path plugins/feishu-plugin/Cargo.toml
	cargo build --manifest-path plugins/cloudflare-tunnel/Cargo.toml

install-plugins: plugins ## Install plugins to ~/.rtb/plugins/
	@mkdir -p ~/.rtb/plugins/feishu-im ~/.rtb/plugins/cloudflare-tunnel
	@cp plugins/feishu-plugin/target/debug/feishu-plugin ~/.rtb/plugins/feishu-im/
	@cp plugins/feishu-plugin/plugin.toml ~/.rtb/plugins/feishu-im/plugin.toml
	@cp plugins/cloudflare-tunnel/target/debug/cloudflare-tunnel ~/.rtb/plugins/cloudflare-tunnel/
	@cp plugins/cloudflare-tunnel/plugin.toml ~/.rtb/plugins/cloudflare-tunnel/plugin.toml
	@echo "Plugins installed to ~/.rtb/plugins/"

# ── Desktop ──────────────────────────────────────────────────
desktop: web ## Build Tauri desktop app
	npx @tauri-apps/cli build --debug --bundles app
	@echo "\n  Built: target/debug/bundle/macos/RTB.app"

# ── Quality ──────────────────────────────────────────────────
test: ## Run all tests
	cargo test --workspace

clean: ## Remove build artifacts
	cargo clean
	rm -rf web/dist
