type Channel = 'browser' | 'feishu';

interface NotificationConfig {
  onBrowserPush: (sessionId: string, event: string, message: string) => void;
  onFeishuPush: (sessionId: string, event: string, message: string) => void;
}

export class NotificationManager {
  private channels: Record<Channel, boolean> = { browser: false, feishu: false };

  constructor(private config: NotificationConfig) {}

  setChannelEnabled(channel: Channel, enabled: boolean): void {
    this.channels[channel] = enabled;
  }

  getChannelStatus(): Record<Channel, boolean> {
    return { ...this.channels };
  }

  notify(sessionId: string, event: string, message: string): void {
    if (this.channels.browser) {
      this.config.onBrowserPush(sessionId, event, message);
    }
    if (this.channels.feishu) {
      this.config.onFeishuPush(sessionId, event, message);
    }
  }
}
