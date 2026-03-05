import { describe, it, expect, afterEach } from 'vitest';
import { SessionManager } from './session-manager.js';

describe('SessionManager', () => {
  let manager: SessionManager;

  afterEach(() => {
    manager?.destroyAll();
  });

  it('creates a session with unique id', () => {
    manager = new SessionManager();
    const session = manager.create('test-echo', 'echo', ['hello']);
    expect(session.id).toBeTruthy();
    expect(session.name).toBe('test-echo');
    expect(session.command).toBe('echo');
    expect(session.status).toBe('running');
  });

  it('lists all sessions', () => {
    manager = new SessionManager();
    manager.create('s1', 'echo', ['1']);
    manager.create('s2', 'echo', ['2']);
    const list = manager.list();
    expect(list.length).toBe(2);
  });

  it('gets a session by id', () => {
    manager = new SessionManager();
    const session = manager.create('find-me', 'echo', ['hi']);
    const found = manager.get(session.id);
    expect(found).toBeTruthy();
    expect(found!.name).toBe('find-me');
  });

  it('removes a session', () => {
    manager = new SessionManager();
    const session = manager.create('remove-me', '', []);
    manager.remove(session.id);
    expect(manager.get(session.id)).toBeUndefined();
  });

  it('session runs inside a shell (stays running after command completes)', async () => {
    manager = new SessionManager();
    const session = manager.create('shell-test', 'echo', ['hello-from-shell']);
    const pty = manager.getPty(session.id)!;

    const output: string[] = [];
    pty.onData((data) => output.push(data));
    await new Promise((resolve) => setTimeout(resolve, 1000));

    const allOutput = output.join('');
    const updated = manager.get(session.id);
    expect(updated?.status).toBe('running');
    expect(allOutput).toContain('hello-from-shell');
  });

  it('creates a plain shell session when command is empty', async () => {
    manager = new SessionManager();
    const session = manager.create('plain-shell', '', []);
    expect(session.status).toBe('running');

    const pty = manager.getPty(session.id)!;
    const output: string[] = [];
    pty.onData((data) => output.push(data));

    await new Promise((resolve) => setTimeout(resolve, 300));
    pty.write('echo plain-shell-works\n');
    await new Promise((resolve) => setTimeout(resolve, 500));

    const allOutput = output.join('');
    expect(allOutput).toContain('plain-shell-works');
  });
});
