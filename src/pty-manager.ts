import * as pty from 'node-pty';

type DataCallback = (data: string) => void;
type ExitCallback = (code: number) => void;

export class PtyManager {
  private process: pty.IPty | null = null;
  private dataCallbacks: DataCallback[] = [];
  private exitCallbacks: ExitCallback[] = [];

  spawn(command: string, args: string[], cols = 80, rows = 24): void {
    // Clean env: remove CLAUDECODE to allow nested Claude Code sessions
    const env = { ...process.env } as Record<string, string>;
    delete env.CLAUDECODE;

    this.process = pty.spawn(command, args, {
      name: 'xterm-256color',
      cols,
      rows,
      cwd: process.cwd(),
      env,
    });

    this.process.onData((data) => {
      for (const cb of this.dataCallbacks) cb(data);
    });

    this.process.onExit(({ exitCode }) => {
      for (const cb of this.exitCallbacks) cb(exitCode);
    });
  }

  onData(callback: DataCallback): void {
    this.dataCallbacks.push(callback);
  }

  onExit(callback: ExitCallback): void {
    this.exitCallbacks.push(callback);
  }

  write(data: string): void {
    this.process?.write(data);
  }

  resize(cols: number, rows: number): void {
    this.process?.resize(cols, rows);
  }

  kill(): void {
    this.process?.kill();
    this.process = null;
  }
}
