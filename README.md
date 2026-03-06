# RTB — Remote Terminal Bridge

[![CI](https://github.com/codeweiz/remote-desktop-control/actions/workflows/ci.yml/badge.svg)](https://github.com/codeweiz/remote-desktop-control/actions/workflows/ci.yml)
[![Release](https://github.com/codeweiz/remote-desktop-control/actions/workflows/release.yml/badge.svg)](https://github.com/codeweiz/remote-desktop-control/releases)

**[中文文档](./README.zh-CN.md)**

Access and manage terminal sessions from anywhere — via web browser, mobile app, or Feishu bot. RTB runs on your server as a single binary with zero dependencies.

## Features

- **Web Terminal** — Full-featured terminal in the browser powered by xterm.js
- **Mobile App** — iOS/Android app with QR code scanning for instant connection
- **Multi-Session** — Create, switch, and manage multiple terminal sessions
- **Remote Access** — Built-in Cloudflare Tunnel support for secure public access
- **Feishu Bot** — Execute commands and receive notifications via Feishu/Lark
- **Single Binary** — Download one file, run it. No Node.js or dependencies needed
- **Token Auth** — Auto-generated token for secure access
- **REST API** — Integrate with your own tools and workflows

## Architecture

```
┌──────────────┐   WebSocket   ┌─────────────────┐   node-pty   ┌───────────┐
│  Web Panel   │◄─────────────►│                 │◄────────────►│  Shell /  │
│  (xterm.js)  │               │   RTB Server    │              │  Process  │
├──────────────┤               │   (Node.js)     │              └───────────┘
│  Mobile App  │◄─────────────►│                 │
│  (Expo)      │               │  HTTP + WS      │
├──────────────┤               │  Port 3000      │
│  Feishu Bot  │◄─ Long Conn ─►│                 │
└──────────────┘               └────────┬────────┘
                                        │
                                 Cloudflare Tunnel
                                   (optional)
```

---

## Quick Start

### Install

**Option 1: Pre-built binary (recommended)**

Download from [GitHub Releases](https://github.com/codeweiz/remote-desktop-control/releases) — single file, no dependencies:

```bash
# macOS (Apple Silicon & Intel via Rosetta 2)
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-darwin-arm64.tar.gz | tar xz

# Linux x64
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-linux-x64.tar.gz | tar xz

# Linux ARM64 (Raspberry Pi, etc.)
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-linux-arm64.tar.gz | tar xz
```

**Option 2: Build from source (requires Node.js 22+)**

```bash
git clone https://github.com/codeweiz/remote-desktop-control.git
cd remote-desktop-control
make install   # install dependencies
make start     # build and start
```

### Start the Server

```bash
# Basic start (manage sessions via web panel)
./rtb start

# Start with a specific command session
./rtb start claude

# Custom port
./rtb start --port 8080

# Enable Cloudflare Tunnel for public access
./rtb start --tunnel
```

On startup, RTB prints the access URL (with auth token) and a **QR code**:

```
Remote Terminal Bridge v2 started!
  Web Panel:    http://192.168.1.100:3000?token=xxx
  Local:        http://localhost:3000?token=xxx

  Mobile: scan QR code to connect
  ██████████████████
  ██ QR Code here ██
  ██████████████████
```

- **Web**: Open the URL in any browser
- **Mobile**: Scan the QR code with the RTB app to auto-connect

### Configuration

Config is stored in `~/.rtb/config.json`:

```bash
./rtb config          # view current config
./rtb config set      # interactive setup
```

Override with CLI flags or environment variables:

| CLI Flag | Env Variable | Description |
|----------|-------------|-------------|
| `--port <port>` | - | HTTP port (default: 3000) |
| `--tunnel` | - | Enable Cloudflare Tunnel |
| `--feishu-app-id <id>` | `FEISHU_APP_ID` | Feishu App ID |
| `--feishu-app-secret <s>` | `FEISHU_APP_SECRET` | Feishu App Secret |
| `--feishu-chat-id <id>` | `FEISHU_CHAT_ID` | Feishu Chat ID (optional) |

Priority: CLI flags > env variables > config file.

### Cloudflare Tunnel (Remote Access)

**Quick tunnel** (temporary domain, no setup):
```bash
./rtb start --tunnel
# Auto-assigns a xxx.trycloudflare.com domain
```

**Named tunnel** (fixed domain, requires setup):
```bash
cloudflared login
cloudflared tunnel create rtb
cloudflared tunnel route dns rtb rtb.your-domain.com
./rtb tunnel setup rtb rtb.your-domain.com
./rtb start --tunnel
```

### Mobile App

RTB includes an iOS/Android app built with Expo (React Native).

**Connect**: Scan the terminal QR code from the app's connect page. Manual address + token input is also supported.

**Download**: Android APK is available on [GitHub Releases](https://github.com/codeweiz/remote-desktop-control/releases).

### REST API

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/sessions` | List all sessions |
| POST | `/api/sessions` | Create session `{name, command, args}` |
| DELETE | `/api/sessions/:id` | Delete session |
| GET | `/api/sessions/buffer?id=` | Get terminal output buffer |
| GET | `/api/notifications` | Get notification settings |
| POST | `/api/notifications` | Update notification settings |
| GET | `/api/settings` | Get server settings |

---

## Development

### Prerequisites

- Node.js 22+
- npm
- Xcode (iOS) / Android Studio (Android) — for mobile dev only

### Project Structure

```
src/                    # Server TypeScript source
  cli.ts                # CLI entry point
  server.ts             # HTTP server + REST API
  ws-server.ts          # WebSocket server
  session-manager.ts    # Session lifecycle
  pty-manager.ts        # node-pty wrapper
  feishu.ts             # Feishu bot integration
  tunnel.ts             # Cloudflare Tunnel
  notification.ts       # Notification manager
  auth.ts               # Token auth
  config.ts             # Config file (~/.rtb/config.json)
web/                    # Web terminal panel (single-file SPA)
mobile/                 # Expo React Native mobile app
build-binary.mjs        # Binary build script (esbuild + Node.js SEA)
```

### Server

```bash
make install            # install dependencies
make dev                # dev mode (tsx)
make build              # compile TypeScript
make start              # build and start
make start-tunnel       # start with Cloudflare Tunnel
make start-claude       # start with a claude session
make test               # run tests (vitest)
```

### Mobile

```bash
make mobile-install     # install mobile dependencies
make mobile-start       # start Expo dev server
make mobile-ios         # run on iOS simulator
make mobile-android     # run on Android emulator
```

### Build Standalone Binary

Uses [Node.js SEA](https://nodejs.org/api/single-executable-applications.html) to package everything into a single executable:

```bash
make build-binary       # outputs release/rtb
```

### Release

Push a version tag to trigger automated builds for all platforms:

```bash
npm version patch       # bump version
git push --follow-tags  # triggers GitHub Actions release
```

---

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Server | Node.js, TypeScript, node-pty, ws |
| Web Panel | xterm.js (single-file SPA) |
| Mobile | Expo, React Native, WebView, expo-camera |
| Tunnel | Cloudflare Tunnel (cloudflared) |
| Messaging | Feishu/Lark SDK (long connection) |
| Testing | Vitest |
| Packaging | esbuild + Node.js SEA |
| CI/CD | GitHub Actions |

## License

MIT
