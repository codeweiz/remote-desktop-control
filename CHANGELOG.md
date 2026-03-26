# Changelog

## [v2.0.0] - 2026-03-27

### RTB 2.0 — Complete Rust Rewrite

RTB 2.0 is a ground-up rewrite of Remote Desktop Control, replacing the original Node.js implementation with a high-performance Rust architecture. The new system is built as a Cargo workspace with modular crates, featuring a tmux-backed terminal, an AI agent system with multi-provider support, a plugin framework, and cross-platform desktop/mobile clients.

---

### Architecture & Core

- **Full Rust rewrite** — replaced the entire Node.js codebase with a Cargo workspace containing 4 crates: `rtb-core`, `rtb-server`, `rtb-cli`, and `rtb-plugin-host`
- **Axum HTTP server** with token-based auth, security middleware, and session management API
- **Event bus** with hybrid channel design for decoupled inter-module communication
- **Config module** with TOML load/save, environment variable overrides, and hot reload
- **Workspace config** (`.rtb.toml`) for per-project settings with file watcher
- **Session store** with JSONLines persistence for fast append-only storage
- **Daemon lifecycle** management with graceful startup/shutdown and token rotation
- **Per-IP rate limiting** and IP blocklist for security hardening
- **Output coalescing**, sparse event index, replay gap handling, and log rotation
- **QR code generation** for quick mobile connection
- **v1 config migration** for seamless upgrade from RTB 1.x

### Terminal System

- **tmux backend** — rewrote PTY session management on top of tmux, replacing the in-process ring buffer with tmux's native scrollback and session multiplexing
- **Binary WebSocket I/O** for efficient terminal data transfer (replaces text-based protocol)
- **OSC 10/11 color query interceptor** for proper terminal theme synchronization
- **capture-pane on connect** so new clients see existing terminal content immediately
- **Canvas renderer fallback** alongside xterm.js for constrained environments
- **WebSocket keepalive** to prevent idle disconnections
- **tmux mouse mode** for native scroll wheel handling in the browser

### Agent System (AI-Powered)

- **ACP SDK integration** — replaced the hand-written agent protocol with the standardized Agent Communication Protocol
- **Multi-provider support** — native subprocess spawning for Claude, Gemini, OpenCode, and Codex agents
- **Claude CLI adapter** with stream-json protocol parser for real-time output
- **Unified AcpBackend** with dedicated thread model per agent session
- **Agent-terminal binding** — agents can control terminal sessions, with cascade delete on cleanup
- **Rich Agent UI** — WebSocket-driven AgentDrawer with real-time streaming, provider selection dialog, and status cards
- **Task Pool dispatcher** for queuing and distributing work across agent sessions
- **Error classification** with user-facing guidance messages
- **Agent message persistence** and auto-restart on failure
- **15-second timeout** on ACP initialization with diagnostic tracing

### Plugin System

- **Plugin host framework** with TOML-based plugin configuration and hot-reload file watcher
- **Feishu (Lark) plugin** — WebSocket long connection for bidirectional message relay, real Feishu API integration (OAuth + send message)
- **Cloudflare Tunnel plugin** — automatic `cloudflared` quick tunnel for zero-config remote access
- **Plugin.toml config passthrough** — `[config]` section forwarded to plugin initialization
- **Server port injection** so plugins can discover the local RTB server

### IM Bridge

- **IM bridge subsystem** wired to EventBus for real-time event forwarding
- **Feishu channel integration** — forward agent output to specific Feishu groups/channels
- **Auto-create agent sessions** on first incoming IM message
- **Bidirectional message sync** — user messages from Feishu/IM displayed in web UI and vice versa
- **Telegram support** scaffolding (alongside Feishu)

### Desktop & Mobile

- **Tauri 2 desktop app** — native macOS/Linux/Windows wrapper with embedded WebView
- **iOS/mobile support** — Tauri lib target for iOS builds, mobile platform schema files
- **Auth token injection** into WebView on startup for seamless authentication
- **Mobile-optimized layout** for agent and terminal views
- **Virtual keyboard bar** with special keys (Ctrl, Alt, Tab, Esc, arrows) for mobile terminal use
- **Plugin status indicators** (Feishu/Tunnel connection state) in the UI

### Frontend

- **React 19 + Vite + Tailwind CSS** — modern frontend stack with fast HMR
- **Terminal-inspired dark theme** — comprehensive UI overhaul with cohesive design language
- **Frontend embedded in binary** via `rust-embed` for single-binary deployment
- **Split-pane layout** with FocusView, agent drawer, and terminal side-by-side
- **CSP headers** configured for Google Fonts and external resources

### CI/CD & Build

- **GitHub Actions CI** — format check, clippy, tests for workspace + plugins + frontend
- **GitHub Actions Release** — automated multi-platform builds triggered by `v*` tags
- **Cross-compilation** — linux-x64, linux-arm64 (via cross-compiler), darwin-arm64
- **Plugin builds** included in release artifacts alongside the main binary
- **Makefile** with targets for desktop, plugins, and development workflows

---

### Migration from v1.x

RTB 2.0 is a complete rewrite. The old Node.js server has been removed. To upgrade:

1. Install the new `rtb` binary from the release artifacts
2. Existing v1 config files will be auto-migrated on first run
3. Plugins (Feishu, Cloudflare Tunnel) are now separate binaries in the `plugins/` directory

---

## [v0.1.1] - Previous Release

See [v0.1.1 release](https://github.com/user/remote-desktop-control/releases/tag/v0.1.1) for details.

## [v0.1.0] - Initial Release

Initial Node.js implementation of Remote Desktop Control.
