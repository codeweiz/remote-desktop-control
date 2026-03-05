.PHONY: help install build dev test test-watch clean \
       start start-tunnel start-claude config config-set tunnel-setup \
       mobile-install mobile-start mobile-ios mobile-android mobile-web \
       mobile-build-dev mobile-build-preview mobile-build-prod

# ── Default ──────────────────────────────────────────────────
help: ## Show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-22s\033[0m %s\n", $$1, $$2}'

# ── Server ───────────────────────────────────────────────────
install: ## Install server dependencies
	npm install

build: ## Build TypeScript to dist/
	npm run build

dev: ## Start server in dev mode (tsx, auto-reload)
	npm run dev -- start

start: build ## Build and start the server
	node dist/cli.js start

start-tunnel: build ## Build and start with Cloudflare Tunnel
	node dist/cli.js start --tunnel

start-claude: build ## Build and start with a claude session
	node dist/cli.js start claude

# ── Config ───────────────────────────────────────────────────
config: ## Show current config
	npx tsx src/cli.ts config

config-set: ## Interactive config setup
	npx tsx src/cli.ts config set

tunnel-setup: ## Setup Cloudflare Named Tunnel (usage: make tunnel-setup NAME=rtb HOST=rtb.example.com)
	npx tsx src/cli.ts tunnel setup $(NAME) $(HOST)

# ── Test ─────────────────────────────────────────────────────
test: ## Run tests
	npm test

test-watch: ## Run tests in watch mode
	npm run test:watch

# ── Mobile (Expo) ────────────────────────────────────────────
mobile-install: ## Install mobile dependencies
	cd mobile && npm install

mobile-start: ## Start Expo dev server
	cd mobile && npx expo start

mobile-ios: ## Run on iOS simulator
	cd mobile && npx expo run:ios

mobile-android: ## Run on Android emulator
	cd mobile && npx expo run:android

mobile-web: ## Run in web browser
	cd mobile && npx expo start --web

mobile-build-dev: ## EAS build (development, internal)
	cd mobile && npx eas build --profile development

mobile-build-preview: ## EAS build (preview APK, internal)
	cd mobile && npx eas build --profile preview

mobile-build-prod: ## EAS build (production)
	cd mobile && npx eas build --profile production

# ── Cleanup ──────────────────────────────────────────────────
clean: ## Remove build artifacts
	rm -rf dist
	rm -rf mobile/.expo
