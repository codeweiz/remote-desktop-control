# RTB — Remote Terminal Bridge

远程终端桥接工具，通过 WebSocket 提供多会话终端管理，支持 Web 面板、移动端 App、飞书机器人和 Cloudflare Tunnel 远程访问。

## 架构

```
┌──────────────┐   WebSocket   ┌─────────────────┐   node-pty   ┌───────────┐
│  Web Panel   │◄─────────────►│                 │◄────────────►│  Shell /  │
│  (xterm.js)  │               │   RTB Server    │              │  Process  │
├──────────────┤               │   (Node.js)     │              └───────────┘
│  Mobile App  │◄─────────────►│                 │
│  (Expo)      │               │  HTTP + WS      │
├──────────────┤               │  Port 3000      │
│  Feishu Bot  │◄─ Long Conn ─►│                 │
└──────────────┘               └────────┬────────┘
                                        │
                                 Cloudflare Tunnel
                                   (可选)
```

**核心模块：**

| 模块 | 说明 |
|------|------|
| `src/server.ts` | HTTP 服务 + REST API |
| `src/ws-server.ts` | WebSocket 服务，终端数据双向传输 |
| `src/session-manager.ts` | 会话生命周期管理 |
| `src/pty-manager.ts` | node-pty 封装，伪终端管理 |
| `src/feishu.ts` | 飞书机器人集成（长连接模式） |
| `src/tunnel.ts` | Cloudflare Tunnel 集成 |
| `src/notification.ts` | 通知管理（浏览器推送 + 飞书） |
| `src/auth.ts` | Token 认证 |
| `src/config.ts` | 配置文件管理（`~/.rtb/config.json`） |
| `web/index.html` | Web 终端面板（单文件 SPA） |
| `mobile/` | Expo React Native 移动端 App |

## 安装

### 下载预编译二进制

从 [GitHub Releases](https://github.com/codeweiz/remote-desktop-control/releases) 下载对应平台的二进制文件：

```bash
# macOS (Apple Silicon)
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-darwin-arm64.tar.gz | tar xz
./rtb start

# macOS (Intel)
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-darwin-x64.tar.gz | tar xz
./rtb start

# Linux (x64)
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-linux-x64.tar.gz | tar xz
./rtb start

# Linux (ARM64)
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-linux-arm64.tar.gz | tar xz
./rtb start
```

### 从源码构建

```bash
make install
make start
```

## 快速开始

```bash
# 安装依赖
make install

# 开发模式启动
make dev

# 构建并启动
make start
```

启动后访问终端输出的 URL（含 token），如：`http://localhost:3000?token=xxx`

服务器启动时会在终端显示 QR Code，Mobile App 扫码即可自动连接。

## 所有命令

运行 `make help` 查看所有可用命令：

```
  install                Install server dependencies
  build                  Build TypeScript to dist/
  build-binary           Build standalone binary for current platform
  dev                    Start server in dev mode (tsx, auto-reload)
  start                  Build and start the server
  start-tunnel           Build and start with Cloudflare Tunnel
  start-claude           Build and start with a claude session
  config                 Show current config
  config-set             Interactive config setup
  tunnel-setup           Setup Cloudflare Named Tunnel
  test                   Run tests
  test-watch             Run tests in watch mode
  mobile-install         Install mobile dependencies
  mobile-start           Start Expo dev server
  mobile-ios             Run on iOS simulator
  mobile-android         Run on Android emulator
  mobile-web             Run in web browser
  mobile-build-dev       EAS build (development, internal)
  mobile-build-preview   EAS build (preview APK, internal)
  mobile-build-prod      EAS build (production)
  clean                  Remove build artifacts
```

## CLI 用法

RTB 提供 `rtb` 命令行工具：

```bash
# 启动服务（通过 Web UI 创建会话）
rtb start

# 启动时自动创建一个 claude 会话
rtb start claude

# 启动时开启 Cloudflare Tunnel
rtb start --tunnel

# 指定端口
rtb start --port 8080

# 飞书集成（CLI 参数 > 环境变量 > 配置文件）
rtb start --feishu-app-id <id> --feishu-app-secret <secret>
```

### 配置管理

配置文件存储在 `~/.rtb/config.json`：

```bash
# 查看当前配置
rtb config

# 交互式配置
rtb config set

# 配置 Cloudflare Named Tunnel
rtb tunnel setup <name> <hostname>
```

### 环境变量

| 变量 | 说明 |
|------|------|
| `FEISHU_APP_ID` | 飞书应用 App ID |
| `FEISHU_APP_SECRET` | 飞书应用 App Secret |
| `FEISHU_CHAT_ID` | 飞书群聊 ID（可选，自动检测） |

## REST API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/sessions` | 获取所有会话列表 |
| POST | `/api/sessions` | 创建新会话 `{name, command, args}` |
| DELETE | `/api/sessions/:id` | 删除会话 |
| GET | `/api/sessions/buffer?id=` | 获取会话终端缓冲区 |
| GET | `/api/notifications` | 获取通知设置 |
| POST | `/api/notifications` | 更新通知设置 `{channel, enabled}` |
| GET | `/api/settings` | 获取服务器设置 |

## 移动端

基于 Expo (React Native) 的移动终端客户端。支持扫码连接：服务器启动后终端会显示 QR Code，在 App 连接页点击 "Scan QR Code" 扫码即可自动连接。

```bash
# 安装依赖
make mobile-install

# 启动开发服务器
make mobile-start

# iOS 模拟器运行
make mobile-ios

# EAS 云构建
make mobile-build-dev       # 开发版（模拟器）
make mobile-build-preview   # 预览版（APK）
make mobile-build-prod      # 正式版
```

## Cloudflare Tunnel

支持两种模式：

1. **快速隧道**（无需配置）：`make start-tunnel`
2. **命名隧道**（固定域名）：
   ```bash
   cloudflared login
   cloudflared tunnel create rtb
   cloudflared tunnel route dns rtb rtb.example.com
   make tunnel-setup NAME=rtb HOST=rtb.example.com
   make start-tunnel
   ```

## 技术栈

- **Server:** Node.js, TypeScript, node-pty, ws
- **Web:** xterm.js (单文件 SPA，无构建步骤)
- **Mobile:** Expo, React Native, WebView
- **Tunnel:** Cloudflare (cloudflared)
- **IM:** 飞书开放平台 SDK (长连接)
- **Test:** Vitest
