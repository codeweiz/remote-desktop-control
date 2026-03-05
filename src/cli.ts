import { startServer } from './server.js';

const args = process.argv.slice(2);
const subcommand = args[0];

function printUsage(): void {
  console.log('Usage:');
  console.log('  rtb start [command]  Start the bridge server');
  console.log('  rtb tunnel setup     Configure Cloudflare Named Tunnel');
  console.log('');
  console.log('Options:');
  console.log('  --port <port>              Web terminal port (default: 3000)');
  console.log('  --tunnel                   Enable Cloudflare Tunnel');
  console.log('  --feishu-app-id <id>       Feishu app ID');
  console.log('  --feishu-app-secret <s>    Feishu app secret');
  console.log('  --feishu-chat-id <id>      Feishu chat ID');
  console.log('  --feishu-webhook-port <p>  Feishu webhook port (default: 3001)');
  console.log('');
  console.log('Examples:');
  console.log('  rtb start                   Start server (create sessions via UI)');
  console.log('  rtb start claude            Start with a claude session');
  console.log('  rtb start bash --tunnel     Start with bash + tunnel');
}

function getOpt(name: string): string | undefined {
  const idx = args.indexOf(name);
  return idx !== -1 ? args[idx + 1] : undefined;
}

if (subcommand === 'tunnel' && args[1] === 'setup') {
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
  const port = parseInt(getOpt('--port') || '3000', 10);
  const tunnel = args.includes('--tunnel');

  const feishuAppId = getOpt('--feishu-app-id') || process.env.FEISHU_APP_ID;
  const feishuAppSecret = getOpt('--feishu-app-secret') || process.env.FEISHU_APP_SECRET;
  const feishuChatId = getOpt('--feishu-chat-id') || process.env.FEISHU_CHAT_ID;
  const feishuWebhookPort = parseInt(
    getOpt('--feishu-webhook-port') || process.env.FEISHU_WEBHOOK_PORT || '3001', 10
  );

  const feishu = feishuAppId && feishuAppSecret && feishuChatId
    ? { appId: feishuAppId, appSecret: feishuAppSecret, chatId: feishuChatId, webhookPort: feishuWebhookPort }
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
