# RTB 2.0 -- Remote Terminal Bridge

**[中文文档](./README.zh-CN.md)**

A high-performance remote terminal bridge built in Rust, providing web-based terminal access with AI agent integration and a plugin-driven architecture.

## Features

- **Remote terminal access** -- Full terminal sessions via web browser and mobile
- **AI Agent management** -- Connect and manage AI agents through the ACP (Agent Communication Protocol)
- **Plugin architecture** -- Extensible IM integrations (Feishu, Telegram, Discord) and tunnel providers
- **Smart notifications** -- 3-layer detection engine (keyword, regex, semantic) for intelligent alerting
- **Task pool** -- Auto-scheduling task queue with priority management
- **Modern UI** -- Dark/light theme, command palette (Ctrl+K), keyboard shortcuts
- **Single binary** -- Self-contained distribution with embedded frontend assets

## Architecture

```
+------------------+     +------------------+
|   CLI (clap)     |     |   Tauri Desktop  |
+--------+---------+     +--------+---------+
         |                        |
         +----------+-------------+
                    |
         +----------v-----------+
         |    Server (Axum)     |
         |  REST + WebSocket    |
         +----------+-----------+
                    |
         +----------v-----------+
         |        Core          |
         |  +------+ +-------+ |
         |  |Event | | PTY   | |
         |  | Bus  | |Manager| |
         |  +------+ +-------+ |
         |  +------+ +-------+ |
         |  |Agent | |Session| |
         |  |  ACP | | Store | |
         |  +------+ +-------+ |
         |  +------+ +-------+ |
         |  |Notif.| | Task  | |
         |  |Engine| | Pool  | |
         |  +------+ +-------+ |
         +-----------+----------+
                     |
         +-----------v----------+
         |    Plugin Host       |
         |  IM / Tunnel plugins |
         +----------------------+
```

## Quick Start

```bash
# Build the frontend
cd web && npm install && npm run build && cd ..

# Build the Rust binary
cargo build --release -p rtb-cli

# Run
./target/release/rtb-cli
```

Then open your browser at `http://localhost:9399`.

Or use the Makefile:

```bash
make build    # Build frontend + release binary
make install  # Build and install to /usr/local/bin/rtb
```

## CLI Usage

```bash
rtb                  # Start the RTB server (foreground)
rtb start -d         # Start as background daemon
rtb stop             # Stop the daemon
rtb status           # Show server status

rtb session list     # List active terminal sessions
rtb session new      # Create a new terminal session
```

## Tech Stack

| Layer    | Technology                          |
|----------|-------------------------------------|
| Backend  | Rust, Tokio, Axum, portable-pty     |
| Frontend | React 19, TypeScript, Tailwind CSS  |
| Terminal | xterm.js                            |
| Desktop  | Tauri 2                             |
| Build    | Cargo workspace, Vite               |

## Development

```bash
# Terminal 1: Start the Rust backend
cargo run -p rtb-cli

# Terminal 2: Start the frontend dev server
cd web && npm run dev
```

Additional commands:

```bash
make test    # Run all tests (Rust + frontend)
make lint    # Run cargo fmt check + clippy
make clean   # Remove build artifacts
```

## Project Structure

```
remote-desktop-control/
+-- crates/
|   +-- core/           # Core library: event bus, PTY, sessions, agents, notifications, task pool
|   +-- server/         # Axum HTTP/WS server, REST API, static file embedding
|   +-- plugin-host/    # Plugin manager, IM and tunnel plugin interfaces
|   +-- cli/            # CLI entry point (clap), daemon lifecycle
+-- web/
|   +-- src/
|   |   +-- components/ # React components (Terminal, SessionList, AgentChat, etc.)
|   |   +-- hooks/      # Custom hooks (useTerminal, useWebSocket, useTheme, etc.)
|   |   +-- lib/        # API client, WebSocket helpers, types
|   +-- index.html
|   +-- tailwind.config.js
|   +-- vite.config.ts
+-- docs/               # Design specs and implementation plans
+-- .github/            # CI/CD workflows
+-- Cargo.toml          # Workspace root
+-- Makefile            # Build commands
+-- LICENSE             # MIT
```

## License

[MIT](LICENSE)
