import * as pty from 'node-pty';

type DataCallback = (data: string) => void;
type ExitCallback = (code: number) => void;

interface PtyOptions {
  bufferSize?: number;
}

export class PtyManager {
  private process: pty.IPty | null = null;
  private dataCallbacks: DataCallback[] = [];
  private exitCallbacks: ExitCallback[] = [];
  private outputLines: string[] = [];
  private currentLine = '';
  private maxLines: number;

  constructor(options?: PtyOptions) {
    this.maxLines = options?.bufferSize ?? 5000;
  }

  spawn(command: string, args: string[], cols = 80, rows = 24, shellCommand?: string): void {
    // Clean env: remove CLAUDECODE to allow nested Claude Code sessions
    const env = { ...process.env } as Record<string, string>;
    delete env.CLAUDECODE;

    // If shellCommand is provided, spawn shell and write command into it
    const shell = process.env.SHELL || '/bin/bash';
    const spawnCmd = shellCommand ? shell : command;
    const spawnArgs = shellCommand ? [] : args;

    this.process = pty.spawn(spawnCmd, spawnArgs, {
      name: 'xterm-256color',
      cols,
      rows,
      cwd: process.cwd(),
      env,
    });

    this.process.onData((data) => {
      this.appendToBuffer(data);
      for (const cb of this.dataCallbacks) cb(data);
    });

    this.process.onExit(({ exitCode }) => {
      for (const cb of this.exitCallbacks) cb(exitCode);
    });

    // Write the command into the shell after a short delay
    if (shellCommand) {
      setTimeout(() => {
        this.process?.write(shellCommand + '\n');
      }, 200);
    }
  }

  private appendToBuffer(data: string): void {
    const parts = data.split('\n');
    this.currentLine += parts[0];
    for (let i = 1; i < parts.length; i++) {
      this.outputLines.push(this.currentLine);
      if (this.outputLines.length > this.maxLines) {
        this.outputLines.shift();
      }
      this.currentLine = parts[i];
    }
  }

  getBuffer(): string {
    return [...this.outputLines, this.currentLine].join('\n');
  }

  getLastLine(): string {
    return this.currentLine;
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

  isRunning(): boolean {
    return this.process !== null;
  }
}
