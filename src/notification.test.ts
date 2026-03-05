import { describe, it, expect, vi } from 'vitest';
import { NotificationManager } from './notification.js';

describe('NotificationManager', () => {
  it('dispatches to enabled channels only', () => {
    const browserFn = vi.fn();
    const feishuFn = vi.fn();
    const nm = new NotificationManager({ onBrowserPush: browserFn, onFeishuPush: feishuFn });
    nm.setChannelEnabled('browser', true);
    nm.setChannelEnabled('feishu', false);
    nm.notify('test-session', 'waiting-input', 'Confirm? (y/n)');
    expect(browserFn).toHaveBeenCalledTimes(1);
    expect(feishuFn).not.toHaveBeenCalled();
  });

  it('dispatches to both channels when enabled', () => {
    const browserFn = vi.fn();
    const feishuFn = vi.fn();
    const nm = new NotificationManager({ onBrowserPush: browserFn, onFeishuPush: feishuFn });
    nm.setChannelEnabled('browser', true);
    nm.setChannelEnabled('feishu', true);
    nm.notify('s1', 'exited', 'Process exited with code 0');
    expect(browserFn).toHaveBeenCalledTimes(1);
    expect(feishuFn).toHaveBeenCalledTimes(1);
  });

  it('does not dispatch when both channels disabled', () => {
    const browserFn = vi.fn();
    const feishuFn = vi.fn();
    const nm = new NotificationManager({ onBrowserPush: browserFn, onFeishuPush: feishuFn });
    nm.notify('s1', 'waiting-input', 'Confirm?');
    expect(browserFn).not.toHaveBeenCalled();
    expect(feishuFn).not.toHaveBeenCalled();
  });

  it('returns channel status', () => {
    const nm = new NotificationManager({ onBrowserPush: vi.fn(), onFeishuPush: vi.fn() });
    nm.setChannelEnabled('browser', true);
    expect(nm.getChannelStatus()).toEqual({ browser: true, feishu: false });
  });
});
