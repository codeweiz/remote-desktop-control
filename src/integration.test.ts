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
