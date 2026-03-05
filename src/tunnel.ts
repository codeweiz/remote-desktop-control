import { spawn } from 'node:child_process';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';

interface TunnelConfig {
  port: number;
  namedTunnel?: string;
  hostname?: string;
}

interface TunnelResult {
  url: string;
}

export function startTunnel(config: TunnelConfig): Promise<TunnelResult> {
  return new Promise((resolve, reject) => {
    const args: string[] = [];

    if (config.namedTunnel) {
      args.push('tunnel', 'run', '--url', `http://localhost:${config.port}`, config.namedTunnel);
      const url = `https://${config.hostname || config.namedTunnel}`;

      const cf = spawn('cloudflared', args, {
        stdio: ['ignore', 'pipe', 'pipe'],
      });

      let started = false;
      const handleOutput = (data: Buffer) => {
        const line = data.toString();
        if (!started && (line.includes('Registered tunnel connection') || line.includes('Connection registered'))) {
          started = true;
          resolve({ url });
        }
      };

      cf.stdout.on('data', handleOutput);
      cf.stderr.on('data', handleOutput);

      cf.on('error', (err) => {
        if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
          reject(new Error('cloudflared not found. Install: brew install cloudflared'));
        } else {
          reject(err);
        }
      });

      setTimeout(() => {
        if (!started) {
          started = true;
          resolve({ url });
        }
      }, 10000);
    } else {
      args.push('tunnel', '--url', `http://localhost:${config.port}`);

      const cf = spawn('cloudflared', args, {
        stdio: ['ignore', 'pipe', 'pipe'],
      });

      let resolved = false;
      const handleOutput = (data: Buffer) => {
        const line = data.toString();
        const match = line.match(/https:\/\/[a-z0-9-]+\.trycloudflare\.com/);
        if (match && !resolved) {
          resolved = true;
          resolve({ url: match[0] });
        }
      };

      cf.stdout.on('data', handleOutput);
      cf.stderr.on('data', handleOutput);

      cf.on('error', (err) => {
        if ((err as NodeJS.ErrnoException).code === 'ENOENT') {
          reject(new Error('cloudflared not found. Install: brew install cloudflared'));
        } else {
          reject(err);
        }
      });

      setTimeout(() => {
        if (!resolved) reject(new Error('Tunnel startup timed out'));
      }, 30000);
    }
  });
}

export function getTunnelConfig(): { name: string; hostname: string } | null {
  const configPath = path.join(os.homedir(), '.rtb', 'tunnel.json');
  if (!fs.existsSync(configPath)) return null;
  try {
    const raw = fs.readFileSync(configPath, 'utf-8');
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

export function saveTunnelConfig(name: string, hostname: string): void {
  const dir = path.join(os.homedir(), '.rtb');
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(
    path.join(dir, 'tunnel.json'),
    JSON.stringify({ name, hostname }, null, 2),
  );
}
