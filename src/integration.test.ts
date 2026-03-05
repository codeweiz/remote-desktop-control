import { describe, it, expect, afterEach } from 'vitest';
import WebSocket from 'ws';
import { SessionManager } from './session-manager.js';
import { WsServer } from './ws-server.js';
import { generateToken } from './auth.js';

describe('integration: SessionManager + WS', () => {
  let sessionManager: SessionManager | null = null;
  let wsServer: WsServer | null = null;

  afterEach(async () => {
    sessionManager?.destroyAll();
    await wsServer?.close();
    sessionManager = null;
    wsServer = null;
  });

  it('relays session PTY output to WS client and accepts input', async () => {
    const token = generateToken();
    sessionManager = new SessionManager();
    wsServer = new WsServer(token);
    await wsServer.startStandalone(9880);

    const session = sessionManager.create('test-cat', 'cat', []);
    const pty = sessionManager.getPty(session.id)!;

    pty.onData((data) => {
      wsServer!.broadcastToSession(session.id, JSON.stringify({ type: 'output', data }));
    });
    wsServer.onInput((sid, data) => {
      sessionManager!.getPty(sid)?.write(data);
    });

    const ws = new WebSocket(`ws://localhost:9880/ws/${session.id}?token=${token}`);
    await new Promise<void>((resolve) => { ws.on('open', resolve); });

    const messages: string[] = [];
    ws.on('message', (raw) => {
      const msg = JSON.parse(raw.toString());
      if (msg.type === 'output') messages.push(msg.data);
    });

    ws.send(JSON.stringify({ type: 'input', data: 'integration-v2-test\n' }));
    await new Promise((resolve) => setTimeout(resolve, 500));

    const allOutput = messages.join('');
    expect(allOutput).toContain('integration-v2-test');
    ws.close();
  });

  it('provides output buffer on reconnect', async () => {
    const token = generateToken();
    sessionManager = new SessionManager();
    wsServer = new WsServer(token);
    await wsServer.startStandalone(9881);

    const session = sessionManager.create('test-buffer', 'echo', ['buffer-replay-test']);
    const pty = sessionManager.getPty(session.id)!;

    pty.onData((data) => {
      wsServer!.broadcastToSession(session.id, JSON.stringify({ type: 'output', data }));
    });

    await new Promise((resolve) => setTimeout(resolve, 500));

    const buffer = pty.getBuffer();
    expect(buffer).toContain('buffer-replay-test');
  });
});
