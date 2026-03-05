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
    const session = manager.create('remove-me', 'cat', []);
    manager.remove(session.id);
    expect(manager.get(session.id)).toBeUndefined();
  });

  it('session updates status to exited', async () => {
    manager = new SessionManager();
    const session = manager.create('will-exit', 'echo', ['done']);
    await new Promise((resolve) => setTimeout(resolve, 500));
    const updated = manager.get(session.id);
    expect(updated?.status).toBe('exited');
  });
});
