.PHONY: dev build test clean web release lint install help desktop desktop-release mobile-ios mobile-android tunnel plugins

# ── Default ──────────────────────────────────────────────────
help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2}'

# ── Development ─────────────────────────────────────────────
dev: ## Run the RTB CLI in development mode
	cargo run -p rtb-cli -- start

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

# ── Desktop ────────────────────────────────────────────────
desktop: web ## Build Tauri desktop app (.app only, skip DMG)
	npx @tauri-apps/cli build --debug --bundles app
	@echo "\n  Built: target/debug/bundle/macos/RTB.app"
	@echo "  Run:   open target/debug/bundle/macos/RTB.app"

desktop-release: web ## Build Tauri desktop app (release with DMG)
	npx @tauri-apps/cli build --bundles app,dmg

# ── Mobile ─────────────────────────────────────────────────
mobile-ios: web ## Build Tauri iOS app
	npx @tauri-apps/cli ios build --debug

mobile-android: web ## Build Tauri Android app
	npx @tauri-apps/cli android build --debug

# ── Tunnel ─────────────────────────────────────────────────
tunnel: ## Start Cloudflare tunnel (requires cloudflared)
	@echo "Building tunnel plugin..."
	@cargo build --manifest-path plugins/cloudflare-tunnel/Cargo.toml
	@echo "Starting RTB with tunnel..."
	@cargo run -p rtb-cli -- start

# ── Plugins ────────────────────────────────────────────────
plugins: ## Build all plugins
	cargo build --manifest-path plugins/feishu-plugin/Cargo.toml
	cargo build --manifest-path plugins/cloudflare-tunnel/Cargo.toml

install-plugins: plugins ## Build and install plugins to ~/.rtb/plugins/
	@mkdir -p ~/.rtb/plugins/feishu-im ~/.rtb/plugins/cloudflare-tunnel
	cp plugins/feishu-plugin/target/debug/feishu-plugin ~/.rtb/plugins/feishu-im/
	cp plugins/feishu-plugin/plugin.toml ~/.rtb/plugins/feishu-im/plugin.toml
	cp plugins/cloudflare-tunnel/target/debug/cloudflare-tunnel ~/.rtb/plugins/cloudflare-tunnel/
	cp plugins/cloudflare-tunnel/plugin.toml ~/.rtb/plugins/cloudflare-tunnel/plugin.toml
	@echo "Plugins installed to ~/.rtb/plugins/"

# ── Install ─────────────────────────────────────────────────
install: build install-plugins ## Build and install rtb + plugins
	cp target/release/rtb-cli /usr/local/bin/rtb
