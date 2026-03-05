import { startServer } from './server.js';
import { loadConfig, saveConfig, getConfigPath } from './config.js';

const args = process.argv.slice(2);
const subcommand = args[0];

function printUsage(): void {
  console.log('Usage:');
  console.log('  rtb start [command]     Start the bridge server');
  console.log('  rtb config              Show current config');
  console.log('  rtb config set          Interactive config setup');
  console.log('  rtb tunnel setup        Configure Cloudflare Named Tunnel');
  console.log('');
  console.log('Options (override config file):');
  console.log('  --port <port>              Web terminal port (default: 3000)');
  console.log('  --tunnel                   Enable Cloudflare Tunnel');
  console.log('  --feishu-app-id <id>       Feishu app ID');
  console.log('  --feishu-app-secret <s>    Feishu app secret');
  console.log('  --feishu-chat-id <id>      Feishu chat ID (optional, auto-detected)');
  console.log('');
  console.log(`Config file: ${getConfigPath()}`);
  console.log('');
  console.log('Examples:');
  console.log('  rtb start                   Start server (create sessions via UI)');
  console.log('  rtb start claude            Start with a claude session');
  console.log('  rtb start --tunnel          Start with tunnel');
}

function getOpt(name: string): string | undefined {
  const idx = args.indexOf(name);
  return idx !== -1 ? args[idx + 1] : undefined;
}

if (subcommand === 'config' && args[1] === 'set') {
  // Interactive config setup
  const readline = await import('node:readline');
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  const ask = (q: string, def?: string): Promise<string> =>
    new Promise(resolve => rl.question(`${q}${def ? ` [${def}]` : ''}: `, answer => resolve(answer.trim() || def || '')));

  const existing = loadConfig();
  console.log('RTB Configuration Setup');
  console.log('=======================');
  console.log('Press Enter to keep current value.\n');

  const port = parseInt(await ask('Port', String(existing.port || 3000)), 10);
  const tunnel = (await ask('Enable tunnel? (yes/no)', existing.tunnel ? 'yes' : 'no')).toLowerCase() === 'yes';

  const setupFeishu = (await ask('Setup Feishu bot? (yes/no)', existing.feishu ? 'yes' : 'no')).toLowerCase() === 'yes';
  let feishu = existing.feishu;
  if (setupFeishu) {
    const appId = await ask('Feishu App ID', existing.feishu?.appId);
    const appSecret = await ask('Feishu App Secret', existing.feishu?.appSecret);
    const chatId = await ask('Feishu Chat ID (optional, auto-detected)', existing.feishu?.chatId || '');
    feishu = { appId, appSecret, ...(chatId ? { chatId } : {}) };
  } else {
    feishu = undefined;
  }

  const config = { port, tunnel, ...(feishu ? { feishu } : {}) };
  saveConfig(config);
  console.log(`\nConfig saved to ${getConfigPath()}`);
  rl.close();
  process.exit(0);

} else if (subcommand === 'config') {
  const config = loadConfig();
  const configPath = getConfigPath();
  console.log(`Config file: ${configPath}\n`);
  if (Object.keys(config).length === 0) {
    console.log('No config file found. Run: rtb config set');
  } else {
    console.log(JSON.stringify(config, null, 2));
  }
  process.exit(0);

} else if (subcommand === 'tunnel' && args[1] === 'setup') {
  const { saveTunnelConfig } = await import('./tunnel.js');
  const name = args[2] || 'rtb';
  const hostname = args[3] || 'rtb.micro-boat.com';
  console.log('Saving tunnel config...');
  console.log(`  Tunnel name: ${name}`);
  console.log(`  Hostname:    ${hostname}`);
  console.log('');
  console.log('Prerequisites:');
  console.log('  1. Run: cloudflared login');
  console.log('  2. Run: cloudflared tunnel create ' + name);
  console.log('  3. Run: cloudflared tunnel route dns ' + name + ' ' + hostname);
  console.log('');
  saveTunnelConfig(name, hostname);
  console.log('Config saved to ~/.rtb/tunnel.json');
  console.log('Now use: rtb start --tunnel');
  process.exit(0);

} else if (subcommand === 'start') {
  // Load config file as base, CLI args override
  const fileConfig = loadConfig();

  const port = parseInt(getOpt('--port') || String(fileConfig.port || 3000), 10);
  const tunnel = args.includes('--tunnel') || fileConfig.tunnel || false;

  // Feishu: CLI args > env vars > config file
  const feishuAppId = getOpt('--feishu-app-id') || process.env.FEISHU_APP_ID || fileConfig.feishu?.appId;
  const feishuAppSecret = getOpt('--feishu-app-secret') || process.env.FEISHU_APP_SECRET || fileConfig.feishu?.appSecret;
  const feishuChatId = getOpt('--feishu-chat-id') || process.env.FEISHU_CHAT_ID || fileConfig.feishu?.chatId;

  const feishu = feishuAppId && feishuAppSecret
    ? { appId: feishuAppId, appSecret: feishuAppSecret, ...(feishuChatId ? { chatId: feishuChatId } : {}) }
    : undefined;

  let initialCommand: { command: string; args: string[] } | undefined;
  if (args[1] && !args[1].startsWith('--')) {
    const commandStr = args[1];
    const commandParts = commandStr.split(' ');
    initialCommand = { command: commandParts[0], args: commandParts.slice(1) };
  }

  startServer({ port, tunnel, initialCommand, feishu }).catch((err) => {
    console.error('Failed to start:', err);
    process.exit(1);
  });
} else {
  printUsage();
  process.exit(1);
}
