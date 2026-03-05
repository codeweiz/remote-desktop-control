import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { OutputThrottler, parseCommand, cleanTerminalOutput } from './feishu.js';

describe('parseCommand', () => {
  it('recognizes /mute', () => {
    expect(parseCommand('/mute')).toEqual({ type: 'mute' });
  });

  it('recognizes /unmute', () => {
    expect(parseCommand('/unmute')).toEqual({ type: 'unmute' });
  });

  it('recognizes /help', () => {
    expect(parseCommand('/help')).toEqual({ type: 'help' });
  });

  it('recognizes /sessions', () => {
    expect(parseCommand('/sessions')).toEqual({ type: 'sessions' });
  });

  it('recognizes /ls as sessions alias', () => {
    expect(parseCommand('/ls')).toEqual({ type: 'sessions' });
  });

  it('recognizes /s <target> to switch session', () => {
    expect(parseCommand('/s my-session')).toEqual({ type: 'switch', target: 'my-session' });
  });

  it('recognizes /new with name only (plain shell)', () => {
    expect(parseCommand('/new dev')).toEqual({ type: 'new', name: 'dev', command: '' });
  });

  it('recognizes /new with name and command', () => {
    expect(parseCommand('/new claude-chat claude --verbose')).toEqual({
      type: 'new', name: 'claude-chat', command: 'claude --verbose',
    });
  });

  it('recognizes /kill', () => {
    expect(parseCommand('/kill old-session')).toEqual({ type: 'kill', target: 'old-session' });
  });

  it('treats other text as terminal input', () => {
    expect(parseCommand('ls -la')).toEqual({ type: 'input', data: 'ls -la' });
  });

  it('treats empty string as input', () => {
    expect(parseCommand('')).toEqual({ type: 'input', data: '' });
  });
});

describe('cleanTerminalOutput', () => {
  it('strips CSI sequences', () => {
    expect(cleanTerminalOutput('\x1b[?2004h\x1b[1mhello\x1b[0m')).toBe('hello');
  });

  it('strips OSC sequences', () => {
    expect(cleanTerminalOutput('\x1b]0;title\x07hello')).toBe('hello');
  });

  it('strips control characters', () => {
    expect(cleanTerminalOutput('hello\x07\x08world')).toBe('helloworld');
  });

  it('collapses multiple blank lines', () => {
    expect(cleanTerminalOutput('a\n\n\n\n\nb')).toBe('a\n\nb');
  });

  it('returns empty for only escape codes', () => {
    expect(cleanTerminalOutput('\x1b[?2004h\x1b[?1004h\x1b[>1u')).toBe('');
  });
});

describe('OutputThrottler', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it('batches output and flushes after interval', async () => {
    const flushed: string[] = [];
    const throttler = new OutputThrottler(2000, (text) => { flushed.push(text); });

    throttler.push('hello ');
    throttler.push('world');

    expect(flushed.length).toBe(0);

    vi.advanceTimersByTime(2000);
    expect(flushed.length).toBe(1);
    expect(flushed[0]).toBe('hello world');
  });

  it('does not flush when muted', () => {
    const flushed: string[] = [];
    const throttler = new OutputThrottler(2000, (text) => { flushed.push(text); });

    throttler.mute();
    throttler.push('secret');
    vi.advanceTimersByTime(2000);

    expect(flushed.length).toBe(0);
  });

  it('resumes flushing after unmute', () => {
    const flushed: string[] = [];
    const throttler = new OutputThrottler(2000, (text) => { flushed.push(text); });

    throttler.mute();
    throttler.push('buffered');
    throttler.unmute();
    throttler.push(' more');
    vi.advanceTimersByTime(2000);

    expect(flushed.length).toBe(1);
    expect(flushed[0]).toBe(' more');
  });

  it('cleans up on destroy', () => {
    const flushed: string[] = [];
    const throttler = new OutputThrottler(2000, (text) => { flushed.push(text); });
    throttler.push('data');
    throttler.destroy();
    vi.advanceTimersByTime(5000);
    expect(flushed.length).toBe(0);
  });
});
