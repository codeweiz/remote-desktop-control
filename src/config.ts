import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

export interface RtbConfig {
  port: number;
  tunnel: boolean;
  feishu?: {
    appId: string;
    appSecret: string;
    chatId?: string;
  };
}

const CONFIG_DIR = path.join(os.homedir(), '.rtb');
const CONFIG_PATH = path.join(CONFIG_DIR, 'config.json');

export function loadConfig(): Partial<RtbConfig> {
  if (!fs.existsSync(CONFIG_PATH)) return {};
  try {
    const raw = fs.readFileSync(CONFIG_PATH, 'utf-8');
    return JSON.parse(raw);
  } catch {
    return {};
  }
}

export function saveConfig(config: Partial<RtbConfig>): void {
  if (!fs.existsSync(CONFIG_DIR)) fs.mkdirSync(CONFIG_DIR, { recursive: true });
  fs.writeFileSync(CONFIG_PATH, JSON.stringify(config, null, 2) + '\n');
}

export function getConfigPath(): string {
  return CONFIG_PATH;
}
