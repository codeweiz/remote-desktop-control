import * as lark from '@larksuiteoapi/node-sdk';

// --- Command parsing ---

interface MuteCommand { type: 'mute'; }
interface UnmuteCommand { type: 'unmute'; }
interface HelpCommand { type: 'help'; }
interface SessionsCommand { type: 'sessions'; }
interface SwitchCommand { type: 'switch'; target: string; }
interface NewCommand { type: 'new'; name: string; command: string; }
interface KillCommand { type: 'kill'; target: string; }
interface InputCommand { type: 'input'; data: string; }
type Command = MuteCommand | UnmuteCommand | HelpCommand | SessionsCommand
  | SwitchCommand | NewCommand | KillCommand | InputCommand;

export function parseCommand(text: string): Command {
  const trimmed = text.trim();
  const lower = trimmed.toLowerCase();
  if (lower === '/mute') return { type: 'mute' };
  if (lower === '/unmute') return { type: 'unmute' };
  if (lower === '/help') return { type: 'help' };
  if (lower === '/sessions' || lower === '/ls') return { type: 'sessions' };
  if (lower.startsWith('/s ')) return { type: 'switch', target: trimmed.slice(3).trim() };
  if (lower.startsWith('/new ')) {
    const rest = trimmed.slice(5).trim();
    const spaceIdx = rest.indexOf(' ');
    if (spaceIdx === -1) return { type: 'new', name: rest, command: '' };
    return { type: 'new', name: rest.slice(0, spaceIdx), command: rest.slice(spaceIdx + 1).trim() };
  }
  if (lower.startsWith('/kill ')) return { type: 'kill', target: trimmed.slice(6).trim() };
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

  isMuted(): boolean {
    return this.muted;
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

// --- Session adapter interface (injected by server) ---

export interface FeishuSessionAdapter {
  listSessions(): { id: string; name: string; status: string }[];
  createSession(name: string, command: string): { id: string; name: string };
  killSession(idOrName: string): boolean;
  writeToSession(idOrName: string, data: string): boolean;
  getSessionIdByName(name: string): string | undefined;
}

// --- Feishu Bot Client (Long Connection) ---

export interface FeishuConfig {
  appId: string;
  appSecret: string;
  chatId?: string;
}

// Strip all terminal escape codes and control characters
export function cleanTerminalOutput(text: string): string {
  let s = text;
  // OSC sequences: ESC ] ... (BEL | ST)
  s = s.replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)?/g, '');
  // CSI sequences: ESC [ (params) (intermediate) (final)
  s = s.replace(/\x1b\[[\x20-\x3f]*[\x30-\x3f]*[\x40-\x7e]/g, '');
  // Other ESC sequences (2-char): ESC + single char
  s = s.replace(/\x1b[^[\]]/g, '');
  // Remaining control chars except newline/tab
  s = s.replace(/[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]/g, '');
  // Collapse 3+ blank lines into 2
  s = s.replace(/\n{3,}/g, '\n\n');
  // Trim leading/trailing whitespace
  s = s.trim();
  return s;
}

const HELP_TEXT = `📋 Commands:
/sessions — list all sessions
/s <name> — switch to session
/new <name> [cmd] — create session
/kill <name> — close session
/mute — pause output
/unmute — resume output
/help — this message

Text without / is sent to the current session.`;

export class FeishuBot {
  private client: lark.Client;
  private throttler: OutputThrottler;
  private chatId: string | undefined;
  private currentSessionId: string | null = null;
  private sessionAdapter: FeishuSessionAdapter | null = null;
  private wsClient: lark.WSClient | null = null;

  constructor(private config: FeishuConfig) {
    this.chatId = config.chatId;
    this.client = new lark.Client({
      appId: config.appId,
      appSecret: config.appSecret,
    });
    this.throttler = new OutputThrottler(5000, (text) => {
      const clean = cleanTerminalOutput(text);
      if (clean.length > 0) {
        this.sendTerminalOutput(clean).catch(console.error);
      }
    });
  }

  async init(): Promise<void> {
    const eventDispatcher = new lark.EventDispatcher({}).register({
      'im.message.receive_v1': (data: any) => {
        this.handleMessageEvent(data);
      },
    });

    this.wsClient = new lark.WSClient({
      appId: this.config.appId,
      appSecret: this.config.appSecret,
      loggerLevel: lark.LoggerLevel.warn,
    });

    await this.wsClient.start({ eventDispatcher });
  }

  setSessionAdapter(adapter: FeishuSessionAdapter): void {
    this.sessionAdapter = adapter;
  }

  setCurrentSession(sessionId: string): void {
    this.currentSessionId = sessionId;
  }

  getCurrentSessionId(): string | null {
    return this.currentSessionId;
  }

  pushOutput(sessionId: string, data: string): void {
    if (this.currentSessionId && sessionId !== this.currentSessionId) return;
    this.throttler.push(data);
  }

  pushSystemMessage(text: string): void {
    this.sendTextMessage(text).catch(console.error);
  }

  private handleMessageEvent(data: any): void {
    const message = data?.message;
    if (!message) return;

    // Auto-detect chat_id
    if (!this.chatId) {
      this.chatId = message.chat_id;
    }

    const msgType = message.message_type;
    if (msgType !== 'text') return;

    let text = '';
    try {
      const content = JSON.parse(message.content || '{}');
      text = content.text || '';
    } catch { return; }

    const cmd = parseCommand(text);
    this.handleCommand(cmd);
  }

  private handleCommand(cmd: Command): void {
    const adapter = this.sessionAdapter;

    switch (cmd.type) {
      case 'mute':
        this.throttler.mute();
        this.sendReply('🔇 Output muted');
        break;

      case 'unmute':
        this.throttler.unmute();
        this.sendReply('🔊 Output unmuted');
        break;

      case 'help':
        this.sendReply(HELP_TEXT);
        break;

      case 'sessions': {
        if (!adapter) { this.sendReply('No session manager'); break; }
        const sessions = adapter.listSessions();
        if (sessions.length === 0) {
          this.sendReply('No active sessions. Use /new <name> [command] to create one.');
          break;
        }
        const lines = sessions.map(s => {
          const marker = s.id === this.currentSessionId ? '→ ' : '  ';
          const statusIcon = s.status === 'running' ? '🟢' : s.status === 'waiting-input' ? '🟡' : '⚫';
          return `${marker}${statusIcon} ${s.name}`;
        });
        this.sendReply(lines.join('\n'));
        break;
      }

      case 'switch': {
        if (!adapter) break;
        const sessions = adapter.listSessions();
        const target = sessions.find(s =>
          s.name.toLowerCase() === cmd.target.toLowerCase() ||
          s.id === cmd.target
        );
        if (!target) {
          this.sendReply(`Session "${cmd.target}" not found. Use /sessions to list.`);
          break;
        }
        this.currentSessionId = target.id;
        this.sendReply(`Switched to → ${target.name}`);
        break;
      }

      case 'new': {
        if (!adapter) break;
        const created = adapter.createSession(cmd.name, cmd.command);
        this.currentSessionId = created.id;
        this.sendReply(`Created session "${created.name}" and switched to it.`);
        break;
      }

      case 'kill': {
        if (!adapter) break;
        const killed = adapter.killSession(cmd.target);
        if (killed) {
          this.sendReply(`Killed session "${cmd.target}".`);
          const remaining = adapter.listSessions();
          if (remaining.length > 0 && !remaining.find(s => s.id === this.currentSessionId)) {
            this.currentSessionId = remaining[0].id;
            this.sendReply(`Auto-switched to → ${remaining[0].name}`);
          } else if (remaining.length === 0) {
            this.currentSessionId = null;
          }
        } else {
          this.sendReply(`Session "${cmd.target}" not found.`);
        }
        break;
      }

      case 'input': {
        if (!this.currentSessionId) {
          if (adapter) {
            const sessions = adapter.listSessions();
            if (sessions.length > 0) {
              this.currentSessionId = sessions[0].id;
            } else {
              this.sendReply('No active sessions. Use /new <name> [command] to create one.');
              break;
            }
          }
        }
        if (this.currentSessionId && adapter) {
          adapter.writeToSession(this.currentSessionId, cmd.data + '\n');
        }
        break;
      }
    }
  }

  private sendReply(text: string): void {
    this.sendTextMessage(text).catch(console.error);
  }

  private async sendTextMessage(text: string): Promise<void> {
    if (!this.chatId) return;
    await this.client.im.message.create({
      params: { receive_id_type: 'chat_id' },
      data: {
        receive_id: this.chatId,
        msg_type: 'text',
        content: JSON.stringify({ text }),
      },
    });
  }

  private async sendTerminalOutput(text: string): Promise<void> {
    if (!this.chatId) return;
    const truncated = text.length > 4000 ? '...' + text.slice(-3997) : text;

    await this.client.im.message.create({
      params: { receive_id_type: 'chat_id' },
      data: {
        receive_id: this.chatId,
        msg_type: 'interactive',
        content: JSON.stringify({
          elements: [{
            tag: 'markdown',
            content: '```\n' + truncated + '\n```',
          }],
        }),
      },
    });
  }

  destroy(): void {
    this.throttler.destroy();
  }
}
