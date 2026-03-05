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
        this.setupConnectionHandler();
        this.httpServer.listen(this.port, () => resolve());
      } else {
        this.wss = new WebSocketServer({ port: this.port }, resolve);
        this.setupConnectionHandler();
      }
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
