# Remote Terminal Bridge Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Node.js bridge server that runs commands in a PTY, exposes them via WebSocket for web terminal access, and integrates with Feishu bot for remote monitoring and interaction.

**Architecture:** A single Node.js process manages a PTY child process, broadcasts output to WebSocket clients and Feishu, and relays input from both channels back to the PTY. Token-based auth secures access.

**Tech Stack:** Node.js, TypeScript, node-pty, ws, xterm.js, Feishu Open Platform HTTP API

---

### Task 1: Project Scaffolding

**Files:**
- Create: `package.json`
- Create: `tsconfig.json`
- Create: `.gitignore`

**Step 1: Initialize git repo**

Run: `cd /Users/zhouwei/Projects/ai/remote-desktop-control && git init`

**Step 2: Create package.json**

```json
{
  "name": "remote-terminal-bridge",
  "version": "0.1.0",
  "description": "Remote terminal bridge with WebSocket and Feishu integration",
  "type": "module",
  "bin": {
    "rtb": "./dist/cli.js"
  },
  "scripts": {
    "build": "tsc",
    "dev": "tsx src/cli.ts",
    "test": "vitest run",
    "test:watch": "vitest"
  },
  "dependencies": {
    "node-pty": "^1.0.0",
    "ws": "^8.18.0"
  },
  "devDependencies": {
    "@types/node": "^22.0.0",
    "@types/ws": "^8.5.0",
    "tsx": "^4.19.0",
    "typescript": "^5.7.0",
    "vitest": "^3.0.0"
  }
}
```

**Step 3: Create tsconfig.json**

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "Node16",
    "moduleResolution": "Node16",
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "declaration": true
  },
  "include": ["src"],
  "exclude": ["node_modules", "dist"]
}
```

**Step 4: Create .gitignore**

```
node_modules/
dist/
.env
```

**Step 5: Install dependencies**

Run: `npm install`
Expected: `node_modules` created, `package-lock.json` generated

**Step 6: Commit**

```bash
git add package.json tsconfig.json .gitignore package-lock.json
git commit -m "chore: project scaffolding with TypeScript, node-pty, ws, vitest"
```

---

### Task 2: Auth Module

**Files:**
- Create: `src/auth.ts`
- Create: `src/auth.test.ts`

**Step 1: Write the failing test**

```typescript
// src/auth.test.ts
import { describe, it, expect } from 'vitest';
import { generateToken, validateToken } from './auth.js';

describe('auth', () => {
  it('generates a 32-char hex token', () => {
    const token = generateToken();
    expect(token).toMatch(/^[a-f0-9]{32}$/);
  });

  it('validates correct token', () => {
    const token = generateToken();
    expect(validateToken(token, token)).toBe(true);
  });

  it('rejects incorrect token', () => {
    const token = generateToken();
    expect(validateToken('wrong', token)).toBe(false);
  });

  it('rejects empty token', () => {
    const token = generateToken();
    expect(validateToken('', token)).toBe(false);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/auth.test.ts`
Expected: FAIL — cannot find module `./auth.js`

**Step 3: Write minimal implementation**

```typescript
// src/auth.ts
import { randomBytes } from 'node:crypto';

export function generateToken(): string {
  return randomBytes(16).toString('hex');
}

export function validateToken(provided: string, expected: string): boolean {
  if (!provided || !expected) return false;
  return provided === expected;
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/auth.test.ts`
Expected: 4 tests PASS

**Step 5: Commit**

```bash
git add src/auth.ts src/auth.test.ts
git commit -m "feat: add token auth module with tests"
```

---

### Task 3: PTY Manager

**Files:**
- Create: `src/pty-manager.ts`
- Create: `src/pty-manager.test.ts`

**Step 1: Write the failing test**

```typescript
// src/pty-manager.test.ts
import { describe, it, expect, afterEach } from 'vitest';
import { PtyManager } from './pty-manager.js';

describe('PtyManager', () => {
  let pty: PtyManager | null = null;

  afterEach(() => {
    pty?.kill();
    pty = null;
  });

  it('spawns a process and receives output', async () => {
    pty = new PtyManager();
    const output: string[] = [];
    pty.onData((data) => output.push(data));
    pty.spawn('echo', ['hello-from-pty']);

    // Wait for output
    await new Promise((resolve) => setTimeout(resolve, 500));
    const joined = output.join('');
    expect(joined).toContain('hello-from-pty');
  });

  it('accepts input', async () => {
    pty = new PtyManager();
    const output: string[] = [];
    pty.onData((data) => output.push(data));
    // Start cat which echoes input
    pty.spawn('cat', []);

    await new Promise((resolve) => setTimeout(resolve, 200));
    pty.write('test-input\n');
    await new Promise((resolve) => setTimeout(resolve, 200));

    const joined = output.join('');
    expect(joined).toContain('test-input');
  });

  it('emits exit event', async () => {
    pty = new PtyManager();
    let exitCode: number | undefined;
    pty.onExit((code) => { exitCode = code; });
    pty.spawn('echo', ['done']);

    await new Promise((resolve) => setTimeout(resolve, 500));
    expect(exitCode).toBeDefined();
  });

  it('supports resize', () => {
    pty = new PtyManager();
    pty.spawn('echo', ['hi']);
    // Should not throw
    expect(() => pty!.resize(120, 40)).not.toThrow();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/pty-manager.test.ts`
Expected: FAIL — cannot find module

**Step 3: Write minimal implementation**

```typescript
// src/pty-manager.ts
import * as pty from 'node-pty';

type DataCallback = (data: string) => void;
type ExitCallback = (code: number) => void;

export class PtyManager {
  private process: pty.IPty | null = null;
  private dataCallbacks: DataCallback[] = [];
  private exitCallbacks: ExitCallback[] = [];

  spawn(command: string, args: string[], cols = 80, rows = 24): void {
    this.process = pty.spawn(command, args, {
      name: 'xterm-256color',
      cols,
      rows,
      cwd: process.cwd(),
      env: process.env as Record<string, string>,
    });

    this.process.onData((data) => {
      for (const cb of this.dataCallbacks) cb(data);
    });

    this.process.onExit(({ exitCode }) => {
      for (const cb of this.exitCallbacks) cb(exitCode);
    });
  }

  onData(callback: DataCallback): void {
    this.dataCallbacks.push(callback);
  }

  onExit(callback: ExitCallback): void {
    this.exitCallbacks.push(callback);
  }

  write(data: string): void {
    this.process?.write(data);
  }

  resize(cols: number, rows: number): void {
    this.process?.resize(cols, rows);
  }

  kill(): void {
    this.process?.kill();
    this.process = null;
  }
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/pty-manager.test.ts`
Expected: 4 tests PASS

**Step 5: Commit**

```bash
git add src/pty-manager.ts src/pty-manager.test.ts
git commit -m "feat: add PTY manager with spawn, I/O, resize, exit handling"
```

---

### Task 4: WebSocket Server

**Files:**
- Create: `src/ws-server.ts`
- Create: `src/ws-server.test.ts`

**Step 1: Write the failing test**

```typescript
// src/ws-server.test.ts
import { describe, it, expect, afterEach } from 'vitest';
import WebSocket from 'ws';
import { WsServer } from './ws-server.js';

describe('WsServer', () => {
  let server: WsServer | null = null;

  afterEach(async () => {
    await server?.close();
    server = null;
  });

  it('rejects connection without valid token', async () => {
    server = new WsServer(9871, 'secret-token');
    await server.start();

    const ws = new WebSocket('ws://localhost:9871?token=wrong');
    const closed = new Promise<number>((resolve) => {
      ws.on('close', (code) => resolve(code));
    });
    const code = await closed;
    expect(code).toBe(1008); // Policy Violation
  });

  it('accepts connection with valid token', async () => {
    server = new WsServer(9872, 'secret-token');
    await server.start();

    const ws = new WebSocket('ws://localhost:9872?token=secret-token');
    const opened = new Promise<boolean>((resolve) => {
      ws.on('open', () => resolve(true));
      ws.on('error', () => resolve(false));
    });
    const result = await opened;
    expect(result).toBe(true);
    ws.close();
  });

  it('broadcasts data to connected clients', async () => {
    server = new WsServer(9873, 'tok');
    await server.start();

    const ws = new WebSocket('ws://localhost:9873?token=tok');
    await new Promise<void>((resolve) => { ws.on('open', resolve); });

    const received: string[] = [];
    ws.on('message', (data) => received.push(data.toString()));

    server.broadcast(JSON.stringify({ type: 'output', data: 'hello' }));
    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(received.length).toBe(1);
    expect(JSON.parse(received[0]).data).toBe('hello');
    ws.close();
  });

  it('emits input from client', async () => {
    server = new WsServer(9874, 'tok');
    await server.start();

    const inputs: string[] = [];
    server.onInput((data) => inputs.push(data));

    const ws = new WebSocket('ws://localhost:9874?token=tok');
    await new Promise<void>((resolve) => { ws.on('open', resolve); });

    ws.send(JSON.stringify({ type: 'input', data: 'ls\n' }));
    await new Promise((resolve) => setTimeout(resolve, 100));

    expect(inputs).toContain('ls\n');
    ws.close();
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/ws-server.test.ts`
Expected: FAIL — cannot find module

**Step 3: Write minimal implementation**

```typescript
// src/ws-server.ts
import { WebSocketServer, WebSocket } from 'ws';
import { IncomingMessage } from 'node:http';

type InputCallback = (data: string) => void;
type ResizeCallback = (cols: number, rows: number) => void;

export class WsServer {
  private wss: WebSocketServer | null = null;
  private inputCallbacks: InputCallback[] = [];
  private resizeCallbacks: ResizeCallback[] = [];

  constructor(
    private port: number,
    private token: string,
  ) {}

  start(): Promise<void> {
    return new Promise((resolve) => {
      this.wss = new WebSocketServer({ port: this.port }, resolve);

      this.wss.on('connection', (ws: WebSocket, req: IncomingMessage) => {
        const url = new URL(req.url || '', `http://localhost:${this.port}`);
        const clientToken = url.searchParams.get('token');

        if (clientToken !== this.token) {
          ws.close(1008, 'Invalid token');
          return;
        }

        ws.on('message', (raw) => {
          try {
            const msg = JSON.parse(raw.toString());
            if (msg.type === 'input' && typeof msg.data === 'string') {
              for (const cb of this.inputCallbacks) cb(msg.data);
            } else if (msg.type === 'resize' && msg.cols && msg.rows) {
              for (const cb of this.resizeCallbacks) cb(msg.cols, msg.rows);
            }
          } catch {
            // ignore malformed messages
          }
        });
      });
    });
  }

  onInput(callback: InputCallback): void {
    this.inputCallbacks.push(callback);
  }

  onResize(callback: ResizeCallback): void {
    this.resizeCallbacks.push(callback);
  }

  broadcast(message: string): void {
    if (!this.wss) return;
    for (const client of this.wss.clients) {
      if (client.readyState === WebSocket.OPEN) {
        client.send(message);
      }
    }
  }

  close(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.wss) return resolve();
      // Close all connected clients first
      for (const client of this.wss.clients) {
        client.terminate();
      }
      this.wss.close((err) => (err ? reject(err) : resolve()));
      this.wss = null;
    });
  }
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/ws-server.test.ts`
Expected: 4 tests PASS

**Step 5: Commit**

```bash
git add src/ws-server.ts src/ws-server.test.ts
git commit -m "feat: add WebSocket server with token auth, broadcast, input relay"
```

---

### Task 5: Web Terminal Frontend

**Files:**
- Create: `web/index.html`

**Step 1: Create the single-file web terminal**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no" />
  <title>Remote Terminal</title>
  <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@xterm/xterm@5/css/xterm.css" />
  <style>
    * { margin: 0; padding: 0; box-sizing: border-box; }
    html, body { height: 100%; background: #1e1e1e; overflow: hidden; }
    #terminal { height: 100%; width: 100%; }
    #status {
      position: fixed; top: 8px; right: 8px; z-index: 10;
      padding: 4px 10px; border-radius: 4px; font-size: 12px;
      font-family: monospace; color: #fff;
    }
    .connected { background: #2ea043; }
    .disconnected { background: #da3633; }
    .connecting { background: #d29922; }
  </style>
</head>
<body>
  <div id="status" class="connecting">connecting...</div>
  <div id="terminal"></div>

  <script type="module">
    import { Terminal } from 'https://cdn.jsdelivr.net/npm/@xterm/xterm@5/+esm';
    import { FitAddon } from 'https://cdn.jsdelivr.net/npm/@xterm/addon-fit@0/+esm';
    import { WebLinksAddon } from 'https://cdn.jsdelivr.net/npm/@xterm/addon-web-links@0/+esm';

    const params = new URLSearchParams(location.search);
    const token = params.get('token');
    const statusEl = document.getElementById('status');

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: 'Menlo, Monaco, "Courier New", monospace',
      theme: {
        background: '#1e1e1e',
        foreground: '#d4d4d4',
      },
    });

    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(new WebLinksAddon());
    term.open(document.getElementById('terminal'));
    fitAddon.fit();

    function connect() {
      const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
      const ws = new WebSocket(`${protocol}//${location.host}/ws?token=${token}`);

      ws.onopen = () => {
        statusEl.textContent = 'connected';
        statusEl.className = 'connected';
        // Send initial terminal size
        ws.send(JSON.stringify({ type: 'resize', cols: term.cols, rows: term.rows }));
      };

      ws.onmessage = (event) => {
        const msg = JSON.parse(event.data);
        if (msg.type === 'output') {
          term.write(msg.data);
        }
      };

      ws.onclose = () => {
        statusEl.textContent = 'disconnected - reconnecting...';
        statusEl.className = 'disconnected';
        setTimeout(connect, 2000);
      };

      ws.onerror = () => ws.close();

      term.onData((data) => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ type: 'input', data }));
        }
      });

      term.onResize(({ cols, rows }) => {
        if (ws.readyState === WebSocket.OPEN) {
          ws.send(JSON.stringify({ type: 'resize', cols, rows }));
        }
      });
    }

    window.addEventListener('resize', () => fitAddon.fit());
    connect();
  </script>
</body>
</html>
```

**Step 2: Manually verify by opening in browser (visual check later during integration)**

No automated test for this file — it's a frontend single file verified during integration (Task 7).

**Step 3: Commit**

```bash
git add web/index.html
git commit -m "feat: add web terminal frontend with xterm.js and auto-reconnect"
```

---

### Task 6: Feishu Bot Integration

**Files:**
- Create: `src/feishu.ts`
- Create: `src/feishu.test.ts`

**Step 1: Write the failing test**

The Feishu module has two testable units: message throttling and command parsing. The actual HTTP calls will be tested during integration.

```typescript
// src/feishu.test.ts
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { OutputThrottler, parseCommand } from './feishu.js';

describe('parseCommand', () => {
  it('recognizes /mute', () => {
    expect(parseCommand('/mute')).toEqual({ type: 'mute' });
  });

  it('recognizes /unmute', () => {
    expect(parseCommand('/unmute')).toEqual({ type: 'unmute' });
  });

  it('treats other text as terminal input', () => {
    expect(parseCommand('ls -la')).toEqual({ type: 'input', data: 'ls -la' });
  });

  it('treats empty string as input', () => {
    expect(parseCommand('')).toEqual({ type: 'input', data: '' });
  });
});

describe('OutputThrottler', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it('batches output and flushes after interval', async () => {
    const flushed: string[] = [];
    const throttler = new OutputThrottler(2000, (text) => { flushed.push(text); });

    throttler.push('hello ');
    throttler.push('world');

    expect(flushed.length).toBe(0);

    vi.advanceTimersByTime(2000);
    expect(flushed.length).toBe(1);
    expect(flushed[0]).toBe('hello world');
  });

  it('does not flush when muted', () => {
    const flushed: string[] = [];
    const throttler = new OutputThrottler(2000, (text) => { flushed.push(text); });

    throttler.mute();
    throttler.push('secret');
    vi.advanceTimersByTime(2000);

    expect(flushed.length).toBe(0);
  });

  it('resumes flushing after unmute', () => {
    const flushed: string[] = [];
    const throttler = new OutputThrottler(2000, (text) => { flushed.push(text); });

    throttler.mute();
    throttler.push('buffered');
    throttler.unmute();
    throttler.push(' more');
    vi.advanceTimersByTime(2000);

    // Only 'more' should be sent (muted content is discarded)
    expect(flushed.length).toBe(1);
    expect(flushed[0]).toBe(' more');
  });

  it('cleans up on destroy', () => {
    const flushed: string[] = [];
    const throttler = new OutputThrottler(2000, (text) => { flushed.push(text); });
    throttler.push('data');
    throttler.destroy();
    vi.advanceTimersByTime(5000);
    expect(flushed.length).toBe(0);
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/feishu.test.ts`
Expected: FAIL — cannot find module

**Step 3: Write implementation**

```typescript
// src/feishu.ts

// --- Command parsing ---

interface MuteCommand { type: 'mute'; }
interface UnmuteCommand { type: 'unmute'; }
interface InputCommand { type: 'input'; data: string; }
type Command = MuteCommand | UnmuteCommand | InputCommand;

export function parseCommand(text: string): Command {
  const trimmed = text.trim().toLowerCase();
  if (trimmed === '/mute') return { type: 'mute' };
  if (trimmed === '/unmute') return { type: 'unmute' };
  return { type: 'input', data: text };
}

// --- Output throttler ---

export class OutputThrottler {
  private buffer = '';
  private timer: ReturnType<typeof setInterval> | null = null;
  private muted = false;

  constructor(
    private intervalMs: number,
    private onFlush: (text: string) => void,
  ) {
    this.timer = setInterval(() => this.flush(), this.intervalMs);
  }

  push(data: string): void {
    if (this.muted) return;
    this.buffer += data;
  }

  mute(): void {
    this.muted = true;
    this.buffer = '';
  }

  unmute(): void {
    this.muted = false;
  }

  private flush(): void {
    if (this.buffer.length === 0) return;
    this.onFlush(this.buffer);
    this.buffer = '';
  }

  destroy(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
    this.buffer = '';
  }
}

// --- Feishu Bot Client ---

interface FeishuConfig {
  appId: string;
  appSecret: string;
  chatId: string;       // Group chat ID to send messages to
  webhookPort: number;   // Port for receiving Feishu event callbacks
}

export class FeishuBot {
  private accessToken = '';
  private tokenExpiry = 0;
  private throttler: OutputThrottler;
  private inputCallback: ((data: string) => void) | null = null;
  private muteCallback: (() => void) | null = null;
  private unmuteCallback: (() => void) | null = null;

  constructor(private config: FeishuConfig) {
    this.throttler = new OutputThrottler(2500, (text) => {
      this.sendMessage(text).catch(console.error);
    });
  }

  async init(): Promise<void> {
    await this.refreshToken();
  }

  pushOutput(data: string): void {
    this.throttler.push(data);
  }

  onInput(callback: (data: string) => void): void {
    this.inputCallback = callback;
  }

  // Handle incoming Feishu event (message from user)
  handleEvent(body: Record<string, unknown>): void {
    // Feishu event v2 structure
    const event = body.event as Record<string, unknown> | undefined;
    if (!event) return;
    const message = event.message as Record<string, unknown> | undefined;
    if (!message) return;

    const content = JSON.parse((message.content as string) || '{}');
    const text = content.text as string || '';

    const cmd = parseCommand(text);
    switch (cmd.type) {
      case 'mute':
        this.throttler.mute();
        break;
      case 'unmute':
        this.throttler.unmute();
        break;
      case 'input':
        this.inputCallback?.(cmd.data + '\n');
        break;
    }
  }

  private async refreshToken(): Promise<void> {
    const res = await fetch('https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        app_id: this.config.appId,
        app_secret: this.config.appSecret,
      }),
    });
    const data = await res.json() as { tenant_access_token: string; expire: number };
    this.accessToken = data.tenant_access_token;
    this.tokenExpiry = Date.now() + (data.expire - 300) * 1000;
  }

  private async getToken(): Promise<string> {
    if (Date.now() >= this.tokenExpiry) {
      await this.refreshToken();
    }
    return this.accessToken;
  }

  private async sendMessage(text: string): Promise<void> {
    const token = await this.getToken();
    // Strip ANSI escape codes for cleaner display in Feishu
    const clean = text.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '');
    // Truncate to avoid hitting Feishu message size limits
    const truncated = clean.length > 4000 ? '...' + clean.slice(-3997) : clean;

    await fetch('https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`,
      },
      body: JSON.stringify({
        receive_id: this.config.chatId,
        msg_type: 'interactive',
        content: JSON.stringify({
          elements: [{
            tag: 'markdown',
            content: '```\n' + truncated + '\n```',
          }],
        }),
      }),
    });
  }

  destroy(): void {
    this.throttler.destroy();
  }
}
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/feishu.test.ts`
Expected: 7 tests PASS

**Step 5: Commit**

```bash
git add src/feishu.ts src/feishu.test.ts
git commit -m "feat: add Feishu bot with throttled output, command parsing, message relay"
```

---

### Task 7: Server Entry Point — Tie Everything Together

**Files:**
- Create: `src/server.ts`

**Step 1: Write the server module**

```typescript
// src/server.ts
import * as http from 'node:http';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PtyManager } from './pty-manager.js';
import { WsServer } from './ws-server.js';
import { FeishuBot } from './feishu.js';
import { generateToken } from './auth.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

interface ServerConfig {
  command: string;
  args: string[];
  port: number;
  feishu?: {
    appId: string;
    appSecret: string;
    chatId: string;
    webhookPort: number;
  };
}

export async function startServer(config: ServerConfig): Promise<void> {
  const token = generateToken();
  const ptyManager = new PtyManager();

  // --- HTTP server for serving web/index.html ---
  const httpServer = http.createServer((req, res) => {
    if (req.url?.startsWith('/ws')) {
      // Handled by WS upgrade
      return;
    }
    // Serve web/index.html for all other routes
    const htmlPath = path.resolve(__dirname, '../web/index.html');
    const html = fs.readFileSync(htmlPath, 'utf-8');
    res.writeHead(200, { 'Content-Type': 'text/html' });
    res.end(html);
  });

  // --- WebSocket server attached to HTTP server ---
  const wsServer = new WsServer(config.port, token, httpServer);
  await wsServer.start();

  // Wire PTY output → WS broadcast
  ptyManager.onData((data) => {
    wsServer.broadcast(JSON.stringify({ type: 'output', data }));
  });

  // Wire WS input → PTY
  wsServer.onInput((data) => ptyManager.write(data));
  wsServer.onResize((cols, rows) => ptyManager.resize(cols, rows));

  // --- Feishu bot (optional) ---
  let feishuBot: FeishuBot | null = null;
  if (config.feishu) {
    feishuBot = new FeishuBot(config.feishu);
    await feishuBot.init();

    // Wire PTY output → Feishu
    ptyManager.onData((data) => feishuBot!.pushOutput(data));

    // Wire Feishu input → PTY
    feishuBot.onInput((data) => ptyManager.write(data));

    // Start Feishu webhook listener
    const feishuServer = http.createServer((req, res) => {
      if (req.method === 'POST') {
        let body = '';
        req.on('data', (chunk) => { body += chunk; });
        req.on('end', () => {
          try {
            const parsed = JSON.parse(body);
            // Handle Feishu URL verification challenge
            if (parsed.challenge) {
              res.writeHead(200, { 'Content-Type': 'application/json' });
              res.end(JSON.stringify({ challenge: parsed.challenge }));
              return;
            }
            feishuBot!.handleEvent(parsed);
          } catch {
            // ignore
          }
          res.writeHead(200);
          res.end('ok');
        });
      } else {
        res.writeHead(404);
        res.end();
      }
    });

    feishuServer.listen(config.feishu.webhookPort, () => {
      console.log(`Feishu webhook listening on port ${config.feishu!.webhookPort}`);
    });
  }

  // --- Start PTY ---
  ptyManager.spawn(config.command, config.args);

  ptyManager.onExit((code) => {
    console.log(`Process exited with code ${code}`);
    feishuBot?.destroy();
    wsServer.close();
    process.exit(code);
  });

  // --- Print access info ---
  const localIP = getLocalIP();
  console.log('');
  console.log('Remote Terminal Bridge started!');
  console.log(`  Web Terminal: http://${localIP}:${config.port}?token=${token}`);
  console.log(`  Local:        http://localhost:${config.port}?token=${token}`);
  if (feishuBot) {
    console.log(`  Feishu:       connected`);
  }
  console.log('');
}

function getLocalIP(): string {
  const { networkInterfaces } = await import('node:os');
  // Defined inline to avoid top-level await
  const nets = networkInterfaces();
  for (const name of Object.keys(nets)) {
    for (const net of nets[name] || []) {
      if (net.family === 'IPv4' && !net.internal) {
        return net.address;
      }
    }
  }
  return 'localhost';
}
```

**Note:** This requires updating `WsServer` to optionally accept an existing `http.Server` instead of creating its own. We'll update it in Step 2.

**Step 2: Update WsServer to support attaching to an HTTP server**

Modify `src/ws-server.ts` — update the constructor and `start()` method:

```typescript
// Updated ws-server.ts constructor and start method
import { WebSocketServer, WebSocket } from 'ws';
import { IncomingMessage } from 'node:http';
import { Server as HttpServer } from 'node:http';

type InputCallback = (data: string) => void;
type ResizeCallback = (cols: number, rows: number) => void;

export class WsServer {
  private wss: WebSocketServer | null = null;
  private inputCallbacks: InputCallback[] = [];
  private resizeCallbacks: ResizeCallback[] = [];

  constructor(
    private port: number,
    private token: string,
    private httpServer?: HttpServer,
  ) {}

  start(): Promise<void> {
    return new Promise((resolve) => {
      if (this.httpServer) {
        this.wss = new WebSocketServer({ server: this.httpServer, path: '/ws' });
        this.httpServer.listen(this.port, () => resolve());
      } else {
        this.wss = new WebSocketServer({ port: this.port }, resolve);
      }
      this.setupConnectionHandler();
    });
  }

  private setupConnectionHandler(): void {
    this.wss!.on('connection', (ws: WebSocket, req: IncomingMessage) => {
      const url = new URL(req.url || '', `http://localhost:${this.port}`);
      const clientToken = url.searchParams.get('token');

      if (clientToken !== this.token) {
        ws.close(1008, 'Invalid token');
        return;
      }

      ws.on('message', (raw) => {
        try {
          const msg = JSON.parse(raw.toString());
          if (msg.type === 'input' && typeof msg.data === 'string') {
            for (const cb of this.inputCallbacks) cb(msg.data);
          } else if (msg.type === 'resize' && msg.cols && msg.rows) {
            for (const cb of this.resizeCallbacks) cb(msg.cols, msg.rows);
          }
        } catch {
          // ignore malformed messages
        }
      });
    });
  }

  onInput(callback: InputCallback): void {
    this.inputCallbacks.push(callback);
  }

  onResize(callback: ResizeCallback): void {
    this.resizeCallbacks.push(callback);
  }

  broadcast(message: string): void {
    if (!this.wss) return;
    for (const client of this.wss.clients) {
      if (client.readyState === WebSocket.OPEN) {
        client.send(message);
      }
    }
  }

  close(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.wss) return resolve();
      for (const client of this.wss.clients) {
        client.terminate();
      }
      this.wss.close((err) => (err ? reject(err) : resolve()));
      this.wss = null;
    });
  }
}
```

**Step 3: Fix the getLocalIP function in server.ts (use synchronous import)**

Replace the `getLocalIP` function:

```typescript
import { networkInterfaces } from 'node:os';

function getLocalIP(): string {
  const nets = networkInterfaces();
  for (const name of Object.keys(nets)) {
    for (const net of nets[name] || []) {
      if (net.family === 'IPv4' && !net.internal) {
        return net.address;
      }
    }
  }
  return 'localhost';
}
```

**Step 4: Run all existing tests to ensure nothing broke**

Run: `npx vitest run`
Expected: All previous tests still pass

**Step 5: Commit**

```bash
git add src/server.ts src/ws-server.ts
git commit -m "feat: add server entry point, wire PTY + WS + Feishu together"
```

---

### Task 8: CLI Entry Point

**Files:**
- Create: `src/cli.ts`

**Step 1: Write the CLI**

```typescript
// src/cli.ts
import { startServer } from './server.js';

const args = process.argv.slice(2);
const subcommand = args[0];

if (subcommand !== 'start' || args.length < 2) {
  console.log('Usage: rtb start <command> [options]');
  console.log('');
  console.log('Options:');
  console.log('  --port <port>              Web terminal port (default: 3000)');
  console.log('  --feishu-app-id <id>       Feishu app ID');
  console.log('  --feishu-app-secret <s>    Feishu app secret');
  console.log('  --feishu-chat-id <id>      Feishu chat ID');
  console.log('  --feishu-webhook-port <p>  Feishu webhook port (default: 3001)');
  console.log('');
  console.log('Examples:');
  console.log('  rtb start claude');
  console.log('  rtb start "claude --model opus"');
  console.log('  rtb start bash --port 8080');
  process.exit(1);
}

// Parse the command — everything after "start" until first --flag
const commandStr = args[1];
const commandParts = commandStr.split(' ');
const command = commandParts[0];
const commandArgs = commandParts.slice(1);

// Parse options
function getOpt(name: string): string | undefined {
  const idx = args.indexOf(name);
  return idx !== -1 ? args[idx + 1] : undefined;
}

const port = parseInt(getOpt('--port') || '3000', 10);

const feishuAppId = getOpt('--feishu-app-id') || process.env.FEISHU_APP_ID;
const feishuAppSecret = getOpt('--feishu-app-secret') || process.env.FEISHU_APP_SECRET;
const feishuChatId = getOpt('--feishu-chat-id') || process.env.FEISHU_CHAT_ID;
const feishuWebhookPort = parseInt(getOpt('--feishu-webhook-port') || process.env.FEISHU_WEBHOOK_PORT || '3001', 10);

const feishu = feishuAppId && feishuAppSecret && feishuChatId
  ? { appId: feishuAppId, appSecret: feishuAppSecret, chatId: feishuChatId, webhookPort: feishuWebhookPort }
  : undefined;

startServer({ command, args: commandArgs, port, feishu }).catch((err) => {
  console.error('Failed to start:', err);
  process.exit(1);
});
```

**Step 2: Test CLI starts correctly**

Run: `npx tsx src/cli.ts start "echo hello" --port 3333`
Expected: Should print "Remote Terminal Bridge started!" with URLs, then "Process exited with code 0"

**Step 3: Commit**

```bash
git add src/cli.ts
git commit -m "feat: add CLI entry point with rtb start command"
```

---

### Task 9: Integration Test — End to End

**Files:**
- Create: `src/integration.test.ts`

**Step 1: Write integration test**

```typescript
// src/integration.test.ts
import { describe, it, expect, afterEach } from 'vitest';
import WebSocket from 'ws';
import { PtyManager } from './pty-manager.js';
import { WsServer } from './ws-server.js';
import { generateToken } from './auth.js';

describe('integration: PTY + WS', () => {
  let ptyManager: PtyManager | null = null;
  let wsServer: WsServer | null = null;

  afterEach(async () => {
    ptyManager?.kill();
    await wsServer?.close();
    ptyManager = null;
    wsServer = null;
  });

  it('relays PTY output to WS client and accepts input', async () => {
    const token = generateToken();
    ptyManager = new PtyManager();
    wsServer = new WsServer(9880, token);
    await wsServer.start();

    // Wire up
    ptyManager.onData((data) => {
      wsServer!.broadcast(JSON.stringify({ type: 'output', data }));
    });
    wsServer.onInput((data) => ptyManager!.write(data));

    // Start a cat process (echo back input)
    ptyManager.spawn('cat', []);

    // Connect WS client
    const ws = new WebSocket(`ws://localhost:9880?token=${token}`);
    await new Promise<void>((resolve) => { ws.on('open', resolve); });

    const messages: string[] = [];
    ws.on('message', (raw) => {
      const msg = JSON.parse(raw.toString());
      if (msg.type === 'output') messages.push(msg.data);
    });

    // Send input through WS
    ws.send(JSON.stringify({ type: 'input', data: 'integration-test\n' }));

    // Wait for output
    await new Promise((resolve) => setTimeout(resolve, 500));
    const allOutput = messages.join('');
    expect(allOutput).toContain('integration-test');

    ws.close();
  });
});
```

**Step 2: Run integration test**

Run: `npx vitest run src/integration.test.ts`
Expected: PASS

**Step 3: Run all tests**

Run: `npx vitest run`
Expected: All tests pass

**Step 4: Commit**

```bash
git add src/integration.test.ts
git commit -m "test: add integration test for PTY + WebSocket relay"
```

---

### Task 10: Final Polish

**Step 1: Add npm link/bin support**

Run: `npm link`
Expected: `rtb` command available globally

**Step 2: Test full flow manually**

Run: `rtb start bash --port 3000`
Then open `http://localhost:3000?token=<printed-token>` on phone browser. Type commands. Verify bidirectional I/O.

**Step 3: Final commit**

```bash
git add -A
git commit -m "chore: finalize project, ready for use"
```

---

## Summary

| Task | Component | Files |
|------|-----------|-------|
| 1 | Project Scaffolding | package.json, tsconfig.json, .gitignore |
| 2 | Auth Module | src/auth.ts, src/auth.test.ts |
| 3 | PTY Manager | src/pty-manager.ts, src/pty-manager.test.ts |
| 4 | WebSocket Server | src/ws-server.ts, src/ws-server.test.ts |
| 5 | Web Terminal | web/index.html |
| 6 | Feishu Bot | src/feishu.ts, src/feishu.test.ts |
| 7 | Server Entry Point | src/server.ts (+ ws-server.ts update) |
| 8 | CLI | src/cli.ts |
| 9 | Integration Test | src/integration.test.ts |
| 10 | Final Polish | manual verification |
