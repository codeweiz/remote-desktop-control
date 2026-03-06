import * as http from 'node:http';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { networkInterfaces } from 'node:os';
import { fileURLToPath } from 'node:url';
import { SessionManager } from './session-manager.js';
import { WsServer } from './ws-server.js';
import { NotificationManager } from './notification.js';
import { FeishuBot, FeishuSessionAdapter } from './feishu.js';
import { startTunnel, getTunnelConfig } from './tunnel.js';
import { generateToken } from './auth.js';
import QRCode from 'qrcode-terminal';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

/** Read a web asset — from SEA embedded assets first, fallback to disk */
function readWebAsset(relativePath: string): string | null {
  try {
    const sea = require('node:sea');
    if (sea.isSea() && sea.getAsset) {
      return sea.getAsset(relativePath, 'utf-8');
    }
  } catch {}
  const diskPath = path.resolve(__dirname, '..', relativePath);
  if (fs.existsSync(diskPath)) {
    return fs.readFileSync(diskPath, 'utf-8');
  }
  return null;
}

export interface ServerConfig {
  port: number;
  tunnel?: boolean;
  initialCommand?: { command: string; args: string[] };
  feishu?: {
    appId: string;
    appSecret: string;
    chatId?: string;
  };
}

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

function wireSessionPty(sessionId: string, sessionManager: SessionManager, wsServer: WsServer, feishuBot: FeishuBot | null): void {
  const pty = sessionManager.getPty(sessionId);
  if (!pty) return;
  pty.onData((data) => {
    wsServer.broadcastToSession(sessionId, JSON.stringify({ type: 'output', data }));
    feishuBot?.pushOutput(sessionId, data);
  });
}

function createSessionAdapter(sessionManager: SessionManager, wsServer: WsServer, feishuBot: FeishuBot): FeishuSessionAdapter {
  return {
    listSessions() {
      return sessionManager.list().map(s => ({ id: s.id, name: s.name, status: s.status }));
    },
    createSession(name: string, command: string) {
      const args = command ? command.split(/\s+/) : [];
      const cmd = args.shift() || '';
      const session = sessionManager.create(name, cmd, args);
      wireSessionPty(session.id, sessionManager, wsServer, feishuBot);
      return { id: session.id, name: session.name };
    },
    killSession(idOrName: string) {
      const sessions = sessionManager.list();
      const target = sessions.find(s =>
        s.name.toLowerCase() === idOrName.toLowerCase() || s.id === idOrName
      );
      if (!target) return false;
      sessionManager.remove(target.id);
      return true;
    },
    writeToSession(idOrName: string, data: string) {
      const sessions = sessionManager.list();
      const target = sessions.find(s =>
        s.name.toLowerCase() === idOrName.toLowerCase() || s.id === idOrName
      );
      if (!target) return false;
      sessionManager.getPty(target.id)?.write(data);
      sessionManager.markActive(target.id);
      return true;
    },
    getSessionIdByName(name: string) {
      const sessions = sessionManager.list();
      return sessions.find(s => s.name.toLowerCase() === name.toLowerCase())?.id;
    },
  };
}

export async function startServer(config: ServerConfig): Promise<void> {
  const token = generateToken();
  const sessionManager = new SessionManager();
  const wsServer = new WsServer(token);

  // Feishu bot (optional)
  let feishuBot: FeishuBot | null = null;
  if (config.feishu) {
    feishuBot = new FeishuBot(config.feishu);
    await feishuBot.init();

    const adapter = createSessionAdapter(sessionManager, wsServer, feishuBot);
    feishuBot.setSessionAdapter(adapter);
  }

  // Notification manager
  const notificationManager = new NotificationManager({
    onBrowserPush: (sessionId, event, message) => {
      wsServer.broadcastAll(JSON.stringify({
        type: 'notification',
        sessionId,
        event,
        message,
      }));
    },
    onFeishuPush: (sessionId, event, message) => {
      const session = sessionManager.get(sessionId);
      const name = session?.name || sessionId;
      feishuBot?.pushSystemMessage(`[${name}] ${event}: ${message}`);
    },
  });

  // Wire session events to notifications
  sessionManager.onEvent((sessionId, event, data) => {
    const session = sessionManager.get(sessionId);
    const name = session?.name || sessionId;
    if (event === 'waiting-input') {
      const lastLine = (data as { lastLine: string })?.lastLine || '';
      notificationManager.notify(sessionId, 'waiting-input', `[${name}] waiting for input: ${lastLine}`);
    } else if (event === 'exited') {
      const code = (data as { code: number })?.code ?? -1;
      notificationManager.notify(sessionId, 'exited', `[${name}] process exited with code ${code}`);
    }
    wsServer.broadcastAll(JSON.stringify({
      type: 'sessions-updated',
      sessions: sessionManager.list(),
    }));
  });

  // HTTP server
  const httpServer = http.createServer((req, res) => {
    const url = new URL(req.url || '', `http://localhost:${config.port}`);

    // JSON API helper
    const jsonResponse = (status: number, data: unknown) => {
      res.writeHead(status, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify(data));
    };

    // API: list sessions (include lastLine from PTY buffer)
    if (url.pathname === '/api/sessions' && req.method === 'GET') {
      const sessions = sessionManager.list().map(s => ({
        ...s,
        lastLine: sessionManager.getPty(s.id)?.getLastLine() || '',
      }));
      return jsonResponse(200, sessions);
    }

    // API: create session
    if (url.pathname === '/api/sessions' && req.method === 'POST') {
      let body = '';
      req.on('data', (chunk: Buffer) => { body += chunk; });
      req.on('end', () => {
        try {
          const { name, command, args: cmdArgs } = JSON.parse(body);
          const session = sessionManager.create(name || command || 'shell', command || '', cmdArgs || []);
          wireSessionPty(session.id, sessionManager, wsServer, feishuBot);
          jsonResponse(201, session);
        } catch {
          res.writeHead(400);
          res.end('Invalid request');
        }
      });
      return;
    }

    // API: delete session
    const deleteMatch = url.pathname.match(/^\/api\/sessions\/([a-zA-Z0-9]+)$/);
    if (deleteMatch && req.method === 'DELETE') {
      sessionManager.remove(deleteMatch[1]);
      res.writeHead(204);
      res.end();
      return;
    }

    // API: get session buffer
    if (url.pathname === '/api/sessions/buffer' && req.method === 'GET') {
      const sessionId = url.searchParams.get('id');
      if (sessionId) {
        const pty = sessionManager.getPty(sessionId);
        jsonResponse(200, { buffer: pty?.getBuffer() || '' });
      } else {
        res.writeHead(400);
        res.end('Missing session id');
      }
      return;
    }

    // API: get notification settings
    if (url.pathname === '/api/notifications' && req.method === 'GET') {
      return jsonResponse(200, notificationManager.getChannelStatus());
    }

    // API: set notification settings
    if (url.pathname === '/api/notifications' && req.method === 'POST') {
      let body = '';
      req.on('data', (chunk: Buffer) => { body += chunk; });
      req.on('end', () => {
        try {
          const { channel, enabled } = JSON.parse(body);
          notificationManager.setChannelEnabled(channel, enabled);
          jsonResponse(200, notificationManager.getChannelStatus());
        } catch {
          res.writeHead(400);
          res.end('Invalid request');
        }
      });
      return;
    }

    // API: get server settings (read-only)
    if (url.pathname === '/api/settings' && req.method === 'GET') {
      const tunnelConfig = getTunnelConfig();
      return jsonResponse(200, {
        port: config.port,
        tunnel: tunnelConfig ? { name: tunnelConfig.name, hostname: tunnelConfig.hostname } : null,
        feishu: config.feishu ? { configured: true, mode: 'long-connection' } : { configured: false },
      });
    }

    // Serve static files
    if (url.pathname === '/commands.json') {
      const content = readWebAsset('web/commands.json');
      if (content) {
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(content);
        return;
      }
    }

    if (url.pathname === '/sw.js') {
      const content = readWebAsset('web/sw.js');
      if (content) {
        res.writeHead(200, { 'Content-Type': 'application/javascript' });
        res.end(content);
        return;
      }
    }

    // Serve index.html for everything else
    const html = readWebAsset('web/index.html');
    if (html) {
      res.writeHead(200, { 'Content-Type': 'text/html' });
      res.end(html);
    } else {
      res.writeHead(500);
      res.end('web/index.html not found');
    }
  });

  // Attach WebSocket
  wsServer.attach(httpServer);

  wsServer.onInput((sessionId, data) => {
    sessionManager.getPty(sessionId)?.write(data);
    sessionManager.markActive(sessionId);
  });
  wsServer.onResize((sessionId, cols, rows) => {
    sessionManager.getPty(sessionId)?.resize(cols, rows);
  });

  // Start HTTP server
  await new Promise<void>((resolve) => {
    httpServer.listen(config.port, resolve);
  });

  // Create initial session if provided
  if (config.initialCommand) {
    const { command, args: cmdArgs } = config.initialCommand;
    const session = sessionManager.create(command, command, cmdArgs);
    wireSessionPty(session.id, sessionManager, wsServer, feishuBot);
    // Auto-set as feishu current session
    if (feishuBot) feishuBot.setCurrentSession(session.id);
  }

  // Print access info
  const localIP = getLocalIP();
  const localUrl = `http://${localIP}:${config.port}?token=${token}`;
  console.log('');
  console.log('Remote Terminal Bridge v2 started!');
  console.log(`  Web Panel:    ${localUrl}`);
  console.log(`  Local:        http://localhost:${config.port}?token=${token}`);

  let tunnelUrl: string | null = null;
  if (config.tunnel) {
    try {
      const tunnelConfig = getTunnelConfig();
      const result = await startTunnel(
        tunnelConfig
          ? { port: config.port, namedTunnel: tunnelConfig.name, hostname: tunnelConfig.hostname }
          : { port: config.port }
      );
      tunnelUrl = `${result.url}?token=${token}`;
      console.log(`  Tunnel:       ${tunnelUrl}`);
    } catch (err) {
      console.error(`  Tunnel:       ${(err as Error).message}`);
    }
  }
  if (config.feishu) {
    console.log(`  Feishu:       connected (long connection)`);
  }

  // QR code for mobile app connection
  const qrAddress = tunnelUrl ? new URL(tunnelUrl).host : `${localIP}:${config.port}`;
  const qrSSL = tunnelUrl ? '1' : '0';
  const mobileLink = `rtb://connect?address=${encodeURIComponent(qrAddress)}&token=${encodeURIComponent(token)}&ssl=${qrSSL}`;
  console.log('');
  console.log('  Mobile: scan QR code to connect');
  QRCode.generate(mobileLink, { small: true }, (code: string) => {
    for (const line of code.split('\n')) {
      console.log(`  ${line}`);
    }
    console.log('');
  });
}
