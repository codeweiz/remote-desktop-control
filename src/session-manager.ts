import { randomBytes } from 'node:crypto';
import { PtyManager } from './pty-manager.js';
import { InputDetector } from './input-detector.js';

export interface Session {
  id: string;
  name: string;
  command: string;
  args: string[];
  status: 'running' | 'exited' | 'waiting-input';
  exitCode?: number;
  createdAt: Date;
  pty: PtyManager;
  detector: InputDetector;
}

export type SessionInfo = Omit<Session, 'pty' | 'detector'>;

type SessionEventCallback = (sessionId: string, event: string, data?: unknown) => void;

export class SessionManager {
  private sessions = new Map<string, Session>();
  private eventCallbacks: SessionEventCallback[] = [];

  create(name: string, command: string, args: string[], cols = 80, rows = 24): SessionInfo {
    const id = randomBytes(8).toString('hex');
    const pty = new PtyManager();

    const detector = new InputDetector({
      idleMs: 5000,
      onWaiting: (lastLine) => {
        const session = this.sessions.get(id);
        if (session) {
          session.status = 'waiting-input';
          this.emit(id, 'waiting-input', { lastLine });
        }
      },
      onExit: (code) => {
        this.emit(id, 'exited', { code });
      },
    });

    const session: Session = {
      id, name, command, args,
      status: 'running',
      createdAt: new Date(),
      pty, detector,
    };

    this.sessions.set(id, session);

    pty.onData((data) => {
      detector.feed(data);
    });

    pty.onExit((code) => {
      session.status = 'exited';
      session.exitCode = code;
      detector.processExited(code);
    });

    // Build the full command string for shell-wrapping
    const fullCommand = args.length > 0
      ? `${command} ${args.map(a => a.includes(' ') ? `"${a}"` : a).join(' ')}`
      : command;

    // Spawn shell and write command into it
    pty.spawn(command, args, cols, rows, fullCommand);
    return this.toInfo(session);
  }

  list(): SessionInfo[] {
    return Array.from(this.sessions.values()).map(s => this.toInfo(s));
  }

  get(id: string): SessionInfo | undefined {
    const session = this.sessions.get(id);
    return session ? this.toInfo(session) : undefined;
  }

  getPty(id: string): PtyManager | undefined {
    return this.sessions.get(id)?.pty;
  }

  getDetector(id: string): InputDetector | undefined {
    return this.sessions.get(id)?.detector;
  }

  markActive(id: string): void {
    const session = this.sessions.get(id);
    if (session && session.status === 'waiting-input') {
      session.status = 'running';
      session.detector.inputProvided();
    }
  }

  remove(id: string): void {
    const session = this.sessions.get(id);
    if (session) {
      session.pty.kill();
      session.detector.destroy();
      this.sessions.delete(id);
    }
  }

  onEvent(callback: SessionEventCallback): void {
    this.eventCallbacks.push(callback);
  }

  destroyAll(): void {
    for (const [id] of this.sessions) {
      this.remove(id);
    }
  }

  private emit(sessionId: string, event: string, data?: unknown): void {
    for (const cb of this.eventCallbacks) cb(sessionId, event, data);
  }

  private toInfo(session: Session): SessionInfo {
    const { pty: _p, detector: _d, ...info } = session;
    return info;
  }
}
