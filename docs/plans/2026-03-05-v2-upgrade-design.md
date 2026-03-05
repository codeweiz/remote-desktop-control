# Remote Terminal Bridge v2 — Upgrade Design

## Goals

Upgrade the base Remote Terminal Bridge with: fixed domain via Cloudflare Named Tunnel, multi-session management, smart notifications (browser push + Feishu) for Claude Code input prompts, quick command buttons with context awareness, output buffer replay, and a modern panel UI.

## 1. Cloudflare Named Tunnel

- Use `cloudflared tunnel` to create a persistent named tunnel bound to `rtb.micro-boat.com`
- Replace current Quick Tunnel (random domain) with Named Tunnel config
- One-time setup: `cloudflared login` → create tunnel → DNS binding → write config
- Add `rtb tunnel setup` CLI command to guide configuration
- On `--tunnel`, use Named Tunnel so URL is always `https://rtb.micro-boat.com?token=xxx`

## 2. Multi-Session Management

### Architecture Change

```
Before: 1 server = 1 PTY = 1 WebSocket
After:  1 server = N PTYs = N WebSocket routes

GET /                    → Session list page
GET /session/:id         → Terminal page
WS  /ws/:id?token=xxx   → Session WebSocket
POST /api/sessions       → Create new session
DELETE /api/sessions/:id → Close session
```

- `SessionManager` manages multiple `PtyManager` instances, each with a unique ID
- Session list shows: name, status (running/exited/waiting-input), start time, command
- CLI: `rtb start` launches server (no single command binding), sessions created via Web UI
- Shortcut preserved: `rtb start claude` starts server and auto-creates one claude session

## 3. Smart Notification System

### Input Detection (Core Feature)

- Monitor terminal output stream for Claude Code waiting patterns:
  - Output stops for 5+ seconds AND last line contains `?`, `(y/n)`, `>` prompt characters
  - Detect Claude Code specific interaction prompts (permission confirmations, file operation confirmations)
- Trigger notification: "Session 'claude' is waiting for your input"

### Process Exit Notification

- Send notification when PTY process exits, include exit code

### Notification Channels

- Browser Push Notification (Service Worker + VAPID keys)
- Feishu bot push
- Both channels independently toggleable via Web UI settings panel

## 4. Quick Command System

### Preset Buttons (Bottom Toolbar)

- Universal: `yes`, `no`, `Ctrl+C`, `Ctrl+D`, `Enter`
- Claude Code specific: `/compact`, `/clear`, `/help`

### Custom Buttons

- Users can add custom commands, stored in `~/.rtb/commands.json`
- Web UI provides add/delete/reorder interface

### Context Awareness

- Waiting for confirmation detected → highlight `yes` / `no` buttons
- Command prompt detected → show common command buttons
- Normal output flowing → buttons collapse to small icon, don't block terminal

## 5. Output Buffer Replay

- Each session keeps last 5000 lines in memory (ring buffer)
- On WebSocket connect, push buffer history first, then switch to real-time stream
- Client renders history data, seamlessly transitions to live output

## 6. Modern Panel UI

```
┌──────────────────────────────────────────────┐
│ connected   rtb.micro-boat.com  [Bell] [Gear]│  ← Top status bar
├──────────┬───────────────────────────────────┤
│ Sessions │                                   │
│          │                                   │
│ > claude │         Terminal Area             │
│   bash   │         (xterm.js)                │
│          │                                   │
│ [+ New]  │                                   │
├──────────┴───────────────────────────────────┤
│ [yes] [no] [Ctrl+C] [/compact] [+ custom]    │  ← Bottom command bar
└──────────────────────────────────────────────┘
```

- Sidebar: session list + new button, collapsible on mobile
- Main: xterm.js terminal
- Bottom: quick command bar
- Top: connection status, notification toggle, settings
- Responsive: sidebar collapses on mobile, opens via hamburger menu

## Project Structure

```
src/
├── server.ts              # Entry point (refactored)
├── session-manager.ts     # Multi-session management (new)
├── pty-manager.ts         # PTY (add output buffer)
├── ws-server.ts           # WS (multi-session routing)
├── notification.ts        # Notification system (new)
├── input-detector.ts      # Input waiting detection (new)
├── tunnel.ts              # Named Tunnel management (new)
├── feishu.ts              # Feishu (refactored for multi-session)
├── auth.ts                # Unchanged
└── cli.ts                 # Updated commands
web/
├── index.html             # Session list + terminal panel (rewritten)
├── sw.js                  # Service Worker for Push (new)
└── commands.json          # Default quick commands (new)
```
