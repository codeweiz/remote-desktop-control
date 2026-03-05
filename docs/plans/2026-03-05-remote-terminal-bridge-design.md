# Remote Terminal Bridge - Design Document

## Problem

Running Claude Code in iTerm2 on desktop requires being physically present to monitor progress and interact. Need a way to monitor and interact with the terminal remotely from a phone browser or Feishu (Lark) messenger.

## Solution

A lightweight Node.js bridge server that:
1. Runs commands (e.g., Claude Code) inside a PTY (pseudo-terminal)
2. Exposes a WebSocket server for real-time terminal access via web browser
3. Integrates with Feishu bot for terminal output forwarding and input relay

## Architecture

```
┌─────────────┐     PTY      ┌──────────────────┐     WS      ┌──────────────┐
│ Claude Code  │◄────────────►│  Bridge Server   │◄───────────►│  Web Terminal │
│ (child proc) │              │  (Node.js)       │             │  (xterm.js)  │
└─────────────┘              │                  │             └──────────────┘
                             │  - PTY Manager   │
                             │  - WS Server     │     HTTP     ┌──────────────┐
                             │  - Feishu Hook   │◄───────────►│  Feishu Bot   │
                             └──────────────────┘             └──────────────┘
```

## Core Components

### 1. PTY Manager (`pty-manager.ts`)
- Uses `node-pty` to spawn a pseudo-terminal
- Starts any command (default: `claude`) as a child process
- Captures all stdout/stderr output
- Accepts and forwards user input to the PTY
- Handles process lifecycle (start, resize, kill)

### 2. WebSocket Server (`ws-server.ts`)
- Real-time bidirectional terminal I/O with web clients
- Pushes PTY output to all connected clients
- Forwards client keyboard input to PTY
- Token-based authentication via URL query parameter
- Supports terminal resize events from clients

### 3. Web Terminal (`web/index.html`)
- Single HTML file, no build step
- Uses xterm.js + xterm-addon-fit for terminal rendering
- Mobile-friendly (works in phone browsers with touch keyboard)
- Connects to WebSocket server with token auth

### 4. Feishu Bot (`feishu.ts`)
- Pushes terminal output to a Feishu chat via HTTP API
- Throttled output: merges output every 2-3 seconds into one message
- Formats output as code blocks for readability
- Receives Feishu message callbacks, forwards user text as PTY input
- Commands: `/mute` to pause output, `/unmute` to resume

### 5. Auth (`auth.ts`)
- Generates a random token on server start
- Validates token on WebSocket connection and Feishu webhook verification
- Personal use only, no user management

## Usage Flow

```bash
# Start the bridge with a command
rtb start "claude"
rtb start "claude --model opus"

# Server outputs:
# Web Terminal: http://192.168.1.x:3000?token=abc123
# Feishu bot connected: ✓

# Open URL on phone browser → full terminal access
# Feishu bot pushes output → reply to interact
```

## Feishu Output Handling

- **Throttling**: Merge output into one message every 2-3 seconds
- **Format**: Feishu rich text with code blocks
- **Mute/Unmute**: `/mute` pauses output push, `/unmute` resumes
- **Input**: Any non-command message is forwarded as terminal input

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Runtime | Node.js + TypeScript |
| PTY | node-pty |
| WebSocket | ws |
| Web Frontend | xterm.js + xterm-addon-fit + vanilla HTML |
| Feishu | Feishu Open Platform HTTP API |
| Auth | Random token generated at startup |

## Project Structure

```
remote-desktop-control/
├── src/
│   ├── server.ts          # Entry point, starts all services
│   ├── pty-manager.ts     # PTY creation and management
│   ├── ws-server.ts       # WebSocket server
│   ├── feishu.ts          # Feishu bot integration
│   └── auth.ts            # Token authentication
├── web/
│   └── index.html         # Web terminal page (single file)
├── package.json
└── tsconfig.json
```

## Security

- Token auth for WebSocket connections
- Intended for personal use on local network or via Tailscale/VPN
- No public internet exposure by default

## Non-Goals (for now)

- Multi-user support
- AI-powered output summarization
- Persistent session history
- Desktop GUI application
