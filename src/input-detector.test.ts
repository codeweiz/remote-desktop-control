import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { InputDetector } from './input-detector.js';

describe('InputDetector', () => {
  beforeEach(() => { vi.useFakeTimers(); });
  afterEach(() => { vi.useRealTimers(); });

  it('detects idle after output stops with prompt pattern', () => {
    const handler = vi.fn();
    const detector = new InputDetector({ idleMs: 3000, onWaiting: handler });

    detector.feed('Compiling files...\n');
    detector.feed('Do you want to proceed? (y/n) ');
    vi.advanceTimersByTime(3000);

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith(expect.stringContaining('proceed'));
  });

  it('does not trigger during active output', () => {
    const handler = vi.fn();
    const detector = new InputDetector({ idleMs: 3000, onWaiting: handler });

    detector.feed('line 1\n');
    vi.advanceTimersByTime(1000);
    detector.feed('line 2\n');
    vi.advanceTimersByTime(1000);
    detector.feed('line 3\n');
    vi.advanceTimersByTime(3000);

    expect(handler).not.toHaveBeenCalled();
  });

  it('detects Claude Code permission prompt', () => {
    const handler = vi.fn();
    const detector = new InputDetector({ idleMs: 3000, onWaiting: handler });

    detector.feed('Claude wants to run: rm -rf temp\n');
    detector.feed('Allow? (y/n) ');
    vi.advanceTimersByTime(3000);

    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('resets after input is provided', () => {
    const handler = vi.fn();
    const detector = new InputDetector({ idleMs: 3000, onWaiting: handler });

    detector.feed('Confirm? (y/n) ');
    vi.advanceTimersByTime(1500);
    detector.inputProvided();
    vi.advanceTimersByTime(3000);

    expect(handler).not.toHaveBeenCalled();
  });

  it('detects process exit', () => {
    const handler = vi.fn();
    const exitHandler = vi.fn();
    const detector = new InputDetector({
      idleMs: 3000,
      onWaiting: handler,
      onExit: exitHandler,
    });

    detector.processExited(0);
    expect(exitHandler).toHaveBeenCalledWith(0);
  });

  it('cleans up on destroy', () => {
    const handler = vi.fn();
    const detector = new InputDetector({ idleMs: 3000, onWaiting: handler });

    detector.feed('Confirm? ');
    detector.destroy();
    vi.advanceTimersByTime(5000);

    expect(handler).not.toHaveBeenCalled();
  });
});
