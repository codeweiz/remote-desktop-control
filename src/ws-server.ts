import { WebSocketServer, WebSocket } from 'ws';
import { IncomingMessage } from 'node:http';
import { Server as HttpServer } from 'node:http';

type InputCallback = (sessionId: string, data: string) => void;
type ResizeCallback = (sessionId: string, cols: number, rows: number) => void;

export class WsServer {
  private wss: WebSocketServer | null = null;
  private inputCallbacks: InputCallback[] = [];
  private resizeCallbacks: ResizeCallback[] = [];
  private sessionClients = new Map<string, Set<WebSocket>>();
  private aliveMap = new WeakMap<WebSocket, boolean>();
  private heartbeatTimer: ReturnType<typeof setInterval> | null = null;

  constructor(
    private token: string,
  ) {}

  attach(httpServer: HttpServer): void {
    this.wss = new WebSocketServer({ server: httpServer });
    this.setupConnectionHandler();
  }

  startStandalone(port: number): Promise<void> {
    return new Promise((resolve) => {
      this.wss = new WebSocketServer({ port }, resolve);
      this.setupConnectionHandler();
    });
  }

  private setupConnectionHandler(): void {
    this.wss!.on('connection', (ws: WebSocket, req: IncomingMessage) => {
      const url = new URL(req.url || '', 'http://localhost');
      const clientToken = url.searchParams.get('token');

      if (clientToken !== this.token) {
        ws.close(1008, 'Invalid token');
        return;
      }

      const pathMatch = url.pathname.match(/^\/ws\/([a-zA-Z0-9]+)$/);
      if (!pathMatch) {
        ws.close(1008, 'Invalid session path');
        return;
      }
      const sessionId = pathMatch[1];

      if (!this.sessionClients.has(sessionId)) {
        this.sessionClients.set(sessionId, new Set());
      }
      this.sessionClients.get(sessionId)!.add(ws);
      this.aliveMap.set(ws, true);

      ws.on('pong', () => {
        this.aliveMap.set(ws, true);
      });

      ws.on('message', (raw) => {
        try {
          const msg = JSON.parse(raw.toString());
          if (msg.type === 'input' && typeof msg.data === 'string') {
            for (const cb of this.inputCallbacks) cb(sessionId, msg.data);
          } else if (msg.type === 'resize' && msg.cols && msg.rows) {
            for (const cb of this.resizeCallbacks) cb(sessionId, msg.cols, msg.rows);
          } else if (msg.type === 'ping') {
            // Application-level ping from mobile client
            this.aliveMap.set(ws, true);
            ws.send(JSON.stringify({ type: 'pong' }));
          }
        } catch { /* ignore */ }
      });

      ws.on('close', () => {
        this.sessionClients.get(sessionId)?.delete(ws);
      });
    });

    // Server-side heartbeat: check every 30s, terminate dead connections
    this.startHeartbeat();
  }

  private startHeartbeat(): void {
    this.heartbeatTimer = setInterval(() => {
      if (!this.wss) return;
      for (const ws of this.wss.clients) {
        if (this.aliveMap.get(ws) === false) {
          ws.terminate();
          continue;
        }
        this.aliveMap.set(ws, false);
        ws.ping();
      }
    }, 30000);
  }

  onInput(callback: InputCallback): void {
    this.inputCallbacks.push(callback);
  }

  onResize(callback: ResizeCallback): void {
    this.resizeCallbacks.push(callback);
  }

  broadcastToSession(sessionId: string, message: string): void {
    const clients = this.sessionClients.get(sessionId);
    if (!clients) return;
    for (const client of clients) {
      if (client.readyState === WebSocket.OPEN) {
        client.send(message);
      }
    }
  }

  broadcastAll(message: string): void {
    if (!this.wss) return;
    for (const client of this.wss.clients) {
      if (client.readyState === WebSocket.OPEN) {
        client.send(message);
      }
    }
  }

  close(): Promise<void> {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
    return new Promise((resolve, reject) => {
      if (!this.wss) return resolve();
      for (const client of this.wss.clients) {
        client.terminate();
      }
      this.wss.close((err) => (err ? reject(err) : resolve()));
      this.wss = null;
      this.sessionClients.clear();
    });
  }
}
