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
    server = new WsServer('secret-token');
    await server.startStandalone(9871);
    const ws = new WebSocket('ws://localhost:9871/ws/abc123?token=wrong');
    const closed = new Promise<number>((resolve) => {
      ws.on('close', (code) => resolve(code));
    });
    expect(await closed).toBe(1008);
  });

  it('rejects connection without session path', async () => {
    server = new WsServer('tok');
    await server.startStandalone(9872);
    const ws = new WebSocket('ws://localhost:9872/ws?token=tok');
    const closed = new Promise<number>((resolve) => {
      ws.on('close', (code) => resolve(code));
    });
    expect(await closed).toBe(1008);
  });

  it('accepts connection with valid token and session path', async () => {
    server = new WsServer('tok');
    await server.startStandalone(9873);
    const ws = new WebSocket('ws://localhost:9873/ws/abc123?token=tok');
    const opened = new Promise<boolean>((resolve) => {
      ws.on('open', () => resolve(true));
      ws.on('error', () => resolve(false));
    });
    expect(await opened).toBe(true);
    ws.close();
  });

  it('broadcasts to correct session only', async () => {
    server = new WsServer('tok');
    await server.startStandalone(9874);

    const ws1 = new WebSocket('ws://localhost:9874/ws/session1?token=tok');
    const ws2 = new WebSocket('ws://localhost:9874/ws/session2?token=tok');
    await Promise.all([
      new Promise<void>((r) => { ws1.on('open', r); }),
      new Promise<void>((r) => { ws2.on('open', r); }),
    ]);

    const msgs1: string[] = [];
    const msgs2: string[] = [];
    ws1.on('message', (d) => msgs1.push(d.toString()));
    ws2.on('message', (d) => msgs2.push(d.toString()));

    server.broadcastToSession('session1', '{"type":"output","data":"for-s1"}');
    await new Promise((r) => setTimeout(r, 100));

    expect(msgs1.length).toBe(1);
    expect(msgs2.length).toBe(0);
    ws1.close();
    ws2.close();
  });

  it('emits input with session id', async () => {
    server = new WsServer('tok');
    await server.startStandalone(9875);

    const inputs: Array<{ sid: string; data: string }> = [];
    server.onInput((sid, data) => inputs.push({ sid, data }));

    const ws = new WebSocket('ws://localhost:9875/ws/mysession?token=tok');
    await new Promise<void>((r) => { ws.on('open', r); });

    ws.send(JSON.stringify({ type: 'input', data: 'hello\n' }));
    await new Promise((r) => setTimeout(r, 100));

    expect(inputs[0].sid).toBe('mysession');
    expect(inputs[0].data).toBe('hello\n');
    ws.close();
  });
});
