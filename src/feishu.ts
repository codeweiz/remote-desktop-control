// --- Command parsing ---

interface MuteCommand { type: 'mute'; }
interface UnmuteCommand { type: 'unmute'; }
interface InputCommand { type: 'input'; data: string; }
type Command = MuteCommand | UnmuteCommand | InputCommand;

export function parseCommand(text: string): Command {
  const trimmed = text.trim().toLowerCase();
  if (trimmed === '/mute') return { type: 'mute' };
  if (trimmed === '/unmute') return { type: 'unmute' };
  return { type: 'input', data: text };
}

// --- Output throttler ---

export class OutputThrottler {
  private buffer = '';
  private timer: ReturnType<typeof setInterval> | null = null;
  private muted = false;

  constructor(
    private intervalMs: number,
    private onFlush: (text: string) => void,
  ) {
    this.timer = setInterval(() => this.flush(), this.intervalMs);
  }

  push(data: string): void {
    if (this.muted) return;
    this.buffer += data;
  }

  mute(): void {
    this.muted = true;
    this.buffer = '';
  }

  unmute(): void {
    this.muted = false;
  }

  private flush(): void {
    if (this.buffer.length === 0) return;
    this.onFlush(this.buffer);
    this.buffer = '';
  }

  destroy(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
    this.buffer = '';
  }
}

// --- Feishu Bot Client ---

interface FeishuConfig {
  appId: string;
  appSecret: string;
  chatId: string;
  webhookPort: number;
}

export class FeishuBot {
  private accessToken = '';
  private tokenExpiry = 0;
  private throttler: OutputThrottler;
  private inputCallback: ((data: string) => void) | null = null;

  constructor(private config: FeishuConfig) {
    this.throttler = new OutputThrottler(2500, (text) => {
      this.sendMessage(text).catch(console.error);
    });
  }

  async init(): Promise<void> {
    await this.refreshToken();
  }

  pushOutput(data: string): void {
    this.throttler.push(data);
  }

  onInput(callback: (data: string) => void): void {
    this.inputCallback = callback;
  }

  handleEvent(body: Record<string, unknown>): void {
    const event = body.event as Record<string, unknown> | undefined;
    if (!event) return;
    const message = event.message as Record<string, unknown> | undefined;
    if (!message) return;

    const content = JSON.parse((message.content as string) || '{}');
    const text = content.text as string || '';

    const cmd = parseCommand(text);
    switch (cmd.type) {
      case 'mute':
        this.throttler.mute();
        break;
      case 'unmute':
        this.throttler.unmute();
        break;
      case 'input':
        this.inputCallback?.(cmd.data + '\n');
        break;
    }
  }

  private async refreshToken(): Promise<void> {
    const res = await fetch('https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        app_id: this.config.appId,
        app_secret: this.config.appSecret,
      }),
    });
    const data = await res.json() as { tenant_access_token: string; expire: number };
    this.accessToken = data.tenant_access_token;
    this.tokenExpiry = Date.now() + (data.expire - 300) * 1000;
  }

  private async getToken(): Promise<string> {
    if (Date.now() >= this.tokenExpiry) {
      await this.refreshToken();
    }
    return this.accessToken;
  }

  private async sendMessage(text: string): Promise<void> {
    const token = await this.getToken();
    const clean = text.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '');
    const truncated = clean.length > 4000 ? '...' + clean.slice(-3997) : clean;

    await fetch('https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${token}`,
      },
      body: JSON.stringify({
        receive_id: this.config.chatId,
        msg_type: 'interactive',
        content: JSON.stringify({
          elements: [{
            tag: 'markdown',
            content: '```\n' + truncated + '\n```',
          }],
        }),
      }),
    });
  }

  destroy(): void {
    this.throttler.destroy();
  }
}
