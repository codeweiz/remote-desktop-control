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
    expect(code).toBe(1008);
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
