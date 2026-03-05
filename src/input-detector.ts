interface InputDetectorConfig {
  idleMs: number;
  onWaiting: (lastLine: string) => void;
  onExit?: (code: number) => void;
}

const PROMPT_PATTERNS = [
  /\?\s*$/,
  /\(y\/n\)\s*$/i,
  /\[Y\/n\]\s*$/,
  /\[yes\/no\]\s*$/i,
  />\s*$/,
  /:\s*$/,
  /Allow\?/i,
  /Confirm\?/i,
  /proceed\?/i,
  /continue\?/i,
  /permission/i,
];

export class InputDetector {
  private timer: ReturnType<typeof setTimeout> | null = null;
  private lastLine = '';
  private config: InputDetectorConfig;
  private notified = false;

  constructor(config: InputDetectorConfig) {
    this.config = config;
  }

  feed(data: string): void {
    const lines = data.split('\n');
    const lastNonEmpty = lines.filter(l => l.trim().length > 0).pop();
    if (lastNonEmpty !== undefined) {
      this.lastLine = lastNonEmpty.replace(/\x1b\[[0-9;]*[a-zA-Z]/g, '');
    }

    this.notified = false;
    this.resetTimer();
  }

  inputProvided(): void {
    this.notified = false;
    this.clearTimer();
  }

  processExited(code: number): void {
    this.clearTimer();
    this.config.onExit?.(code);
  }

  destroy(): void {
    this.clearTimer();
  }

  private resetTimer(): void {
    this.clearTimer();
    this.timer = setTimeout(() => {
      if (!this.notified && this.matchesPromptPattern()) {
        this.notified = true;
        this.config.onWaiting(this.lastLine);
      }
    }, this.config.idleMs);
  }

  private clearTimer(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }

  private matchesPromptPattern(): boolean {
    return PROMPT_PATTERNS.some(pattern => pattern.test(this.lastLine));
  }
}
