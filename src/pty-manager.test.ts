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

    await new Promise((resolve) => setTimeout(resolve, 500));
    const joined = output.join('');
    expect(joined).toContain('hello-from-pty');
  });

  it('accepts input', async () => {
    pty = new PtyManager();
    const output: string[] = [];
    pty.onData((data) => output.push(data));
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
    expect(() => pty!.resize(120, 40)).not.toThrow();
  });
});
