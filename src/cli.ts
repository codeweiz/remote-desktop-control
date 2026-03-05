import { startServer } from './server.js';

const args = process.argv.slice(2);
const subcommand = args[0];

if (subcommand !== 'start' || args.length < 2) {
  console.log('Usage: rtb start <command> [options]');
  console.log('');
  console.log('Options:');
  console.log('  --port <port>              Web terminal port (default: 3000)');
  console.log('  --feishu-app-id <id>       Feishu app ID');
  console.log('  --feishu-app-secret <s>    Feishu app secret');
  console.log('  --feishu-chat-id <id>      Feishu chat ID');
  console.log('  --feishu-webhook-port <p>  Feishu webhook port (default: 3001)');
  console.log('');
  console.log('Examples:');
  console.log('  rtb start claude');
  console.log('  rtb start "claude --model opus"');
  console.log('  rtb start bash --port 8080');
  process.exit(1);
}

// Parse the command
const commandStr = args[1];
const commandParts = commandStr.split(' ');
const command = commandParts[0];
const commandArgs = commandParts.slice(1);

// Parse options
function getOpt(name: string): string | undefined {
  const idx = args.indexOf(name);
  return idx !== -1 ? args[idx + 1] : undefined;
}

const port = parseInt(getOpt('--port') || '3000', 10);

const feishuAppId = getOpt('--feishu-app-id') || process.env.FEISHU_APP_ID;
const feishuAppSecret = getOpt('--feishu-app-secret') || process.env.FEISHU_APP_SECRET;
const feishuChatId = getOpt('--feishu-chat-id') || process.env.FEISHU_CHAT_ID;
const feishuWebhookPort = parseInt(getOpt('--feishu-webhook-port') || process.env.FEISHU_WEBHOOK_PORT || '3001', 10);

const feishu = feishuAppId && feishuAppSecret && feishuChatId
  ? { appId: feishuAppId, appSecret: feishuAppSecret, chatId: feishuChatId, webhookPort: feishuWebhookPort }
  : undefined;

startServer({ command, args: commandArgs, port, feishu }).catch((err) => {
  console.error('Failed to start:', err);
  process.exit(1);
});
