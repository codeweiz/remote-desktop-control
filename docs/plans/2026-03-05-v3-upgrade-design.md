# Remote Terminal Bridge v3 — Upgrade Design

## Goals

1. Fix bug: session command exits → terminal unusable (spawn shell, run command inside it)
2. Fix bug: gear settings button does nothing (implement settings panel)
3. New: React Native mobile app with full feature parity

## 1. Bug Fix: Session Falls Back to Shell

**Problem:** Creating a session with a specific command (e.g., `claude`), when the command exits the terminal becomes unusable.

**Solution:** Sessions always run inside a shell. On session creation:
- Spawn user's default shell (`$SHELL` or `/bin/bash`)
- If a command is specified, write it into the shell after a short delay
- When command exits, user is back in the shell
- Session status only becomes `exited` when the shell itself exits

**Files:** `src/session-manager.ts`

## 2. Settings Panel (Gear Icon)

**Problem:** Gear icon ⚙ is a placeholder with no functionality.

**Solution:** Settings modal with two sections:

**Server config (read-only display):**
- Port, Tunnel domain (read-only, requires restart)
- Feishu status: connected/not configured

**Terminal display config (instant effect):**
- Font size (12-24px, slider)
- Color scheme (3-4 presets: Dark, Monokai, Solarized, Light)
- Saved to `localStorage`, restored on reload

**Files:** `web/index.html`

## 3. React Native App

### Architecture

```
┌─────────────────────────┐
│   React Native App      │
│                         │
│  ┌───────────────────┐  │
│  │ Session List       │  │  ← Native FlatList
│  └───────────────────┘  │
│  ┌───────────────────┐  │
│  │ Terminal           │  │  ← WebView + xterm.js
│  └───────────────────┘  │
│  ┌───────────────────┐  │
│  │ Quick Commands     │  │  ← Native ScrollView + Buttons
│  └───────────────────┘  │
│  ┌───────────────────┐  │
│  │ Settings           │  │  ← Native Screen
│  └───────────────────┘  │
└─────────────────────────┘
         │ HTTP/WS
         ▼
┌─────────────────────────┐
│   Bridge Server (Node)  │
└─────────────────────────┘
```

### Tech Stack

- React Native (Expo)
- React Navigation (Tab + Stack)
- react-native-webview (terminal rendering)
- expo-notifications (native push)

### Screens

| Screen | Content |
|--------|---------|
| Sessions | Session list with status indicators, create/delete |
| Terminal | WebView loading xterm.js, bottom quick command bar |
| Settings | Server address, tunnel domain, Feishu config, notification toggles, terminal font/theme |

### Connection Config

- First launch: guide user to enter server address (e.g. `rtb.micro-boat.com` or `192.168.x.x:3000`) and token
- Saved to AsyncStorage, auto-connect on subsequent launches
- Editable in Settings

### Notifications

- expo-notifications for native push
- Server pushes `notification` WS messages
- App converts to local notification when in background

### Project Structure

```
mobile/
├── app/
│   ├── (tabs)/
│   │   ├── sessions.tsx
│   │   ├── terminal.tsx
│   │   └── settings.tsx
│   ├── _layout.tsx
│   └── connect.tsx
├── components/
│   ├── SessionList.tsx
│   ├── TerminalWebView.tsx
│   ├── QuickCommandBar.tsx
│   └── CreateSessionModal.tsx
├── hooks/
│   ├── useServer.ts
│   └── useWebSocket.ts
├── web/
│   └── terminal.html
├── app.json
├── package.json
└── tsconfig.json
```
