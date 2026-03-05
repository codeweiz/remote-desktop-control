import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { OutputThrottler, parseCommand } from './feishu.js';

describe('parseCommand', () => {
  it('recognizes /mute', () => {
    expect(parseCommand('/mute')).toEqual({ type: 'mute' });
  });

  it('recognizes /unmute', () => {
    expect(parseCommand('/unmute')).toEqual({ type: 'unmute' });
  });

  it('treats other text as terminal input', () => {
    expect(parseCommand('ls -la')).toEqual({ type: 'input', data: 'ls -la' });
  });

  it('treats empty string as input', () => {
    expect(parseCommand('')).toEqual({ type: 'input', data: '' });
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
