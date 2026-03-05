import * as http from 'node:http';
import * as fs from 'node:fs';
import * as path from 'node:path';
import { networkInterfaces } from 'node:os';
import { fileURLToPath } from 'node:url';
import { spawn as cpSpawn } from 'node:child_process';
import { PtyManager } from './pty-manager.js';
import { WsServer } from './ws-server.js';
import { FeishuBot } from './feishu.js';
import { generateToken } from './auth.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export interface ServerConfig {
  command: string;
  args: string[];
  port: number;
  tunnel?: boolean;
  feishu?: {
    appId: string;
    appSecret: string;
    chatId: string;
    webhookPort: number;
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

export async function startServer(config: ServerConfig): Promise<void> {
  const token = generateToken();
  const ptyManager = new PtyManager();

  // HTTP server for serving web/index.html
  const httpServer = http.createServer((_req, res) => {
    const htmlPath = path.resolve(__dirname, '../web/index.html');
    const html = fs.readFileSync(htmlPath, 'utf-8');
    res.writeHead(200, { 'Content-Type': 'text/html' });
    res.end(html);
  });

  // WebSocket server attached to HTTP server
  const wsServer = new WsServer(config.port, token, httpServer);
  await wsServer.start();

  // Wire PTY output → WS broadcast
  ptyManager.onData((data) => {
    wsServer.broadcast(JSON.stringify({ type: 'output', data }));
  });

  // Wire WS input → PTY
  wsServer.onInput((data) => ptyManager.write(data));
  wsServer.onResize((cols, rows) => ptyManager.resize(cols, rows));

  // Feishu bot (optional)
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
        req.on('data', (chunk: Buffer) => { body += chunk; });
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

  // Start PTY
  ptyManager.spawn(config.command, config.args);

  ptyManager.onExit((code) => {
    console.log(`Process exited with code ${code}`);
    feishuBot?.destroy();
    wsServer.close();
    process.exit(code);
  });

  // Print access info
  const localIP = getLocalIP();
  console.log('');
  console.log('Remote Terminal Bridge started!');
  console.log(`  Web Terminal: http://${localIP}:${config.port}?token=${token}`);
  console.log(`  Local:        http://localhost:${config.port}?token=${token}`);
  if (feishuBot) {
    console.log(`  Feishu:       connected`);
  }

  // Cloudflare Tunnel (optional)
  if (config.tunnel) {
    startTunnel(config.port, token);
  }

  console.log('');
}

function startTunnel(port: number, token: string): void {
  const cf = cpSpawn('cloudflared', ['tunnel', '--url', `http://localhost:${port}`], {
    stdio: ['ignore', 'pipe', 'pipe'],
  });

  let tunnelUrl = '';

  const handleOutput = (data: Buffer) => {
    const line = data.toString();
    // cloudflared prints the URL to stderr
    const match = line.match(/https:\/\/[a-z0-9-]+\.trycloudflare\.com/);
    if (match && !tunnelUrl) {
      tunnelUrl = match[0];
      console.log(`  Tunnel:       ${tunnelUrl}?token=${token}`);
    }
  };

  cf.stdout.on('data', handleOutput);
  cf.stderr.on('data', handleOutput);

  cf.on('error', (err) => {
    if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
      console.error('  Tunnel:       cloudflared not found. Install: brew install cloudflared');
    } else {
      console.error('  Tunnel:       failed to start:', err.message);
    }
  });

  cf.on('exit', (code) => {
    if (code !== null && code !== 0) {
      console.error(`  Tunnel exited with code ${code}`);
    }
  });
}
