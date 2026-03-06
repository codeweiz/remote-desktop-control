# RTB — 远程终端桥接工具

[![CI](https://github.com/codeweiz/remote-desktop-control/actions/workflows/ci.yml/badge.svg)](https://github.com/codeweiz/remote-desktop-control/actions/workflows/ci.yml)
[![Release](https://github.com/codeweiz/remote-desktop-control/actions/workflows/release.yml/badge.svg)](https://github.com/codeweiz/remote-desktop-control/releases)

**[English](./README.md)**

在服务器上启动 RTB，即可通过 Web 浏览器、手机 App 或飞书机器人随时随地访问和管理终端会话。单文件二进制，零依赖，开箱即用。

## 特性

- **Web 终端** — 基于 xterm.js 的浏览器终端，完整终端体验
- **手机 App** — iOS/Android 客户端，扫码即连
- **多会话管理** — 创建、切换、管理多个终端会话
- **远程访问** — 内置 Cloudflare Tunnel，一键开启公网访问
- **飞书机器人** — 通过飞书执行命令、接收通知
- **单文件二进制** — 下载即用，无需安装 Node.js 或任何依赖
- **Token 认证** — 自动生成认证 Token，安全访问
- **REST API** — 可与其他工具集成

## 架构

```
┌──────────────┐   WebSocket   ┌─────────────────┐   node-pty   ┌───────────┐
│  Web 面板    │◄─────────────►│                 │◄────────────►│  Shell /  │
│  (xterm.js)  │               │   RTB Server    │              │  Process  │
├──────────────┤               │   (Node.js)     │              └───────────┘
│  手机 App    │◄─────────────►│                 │
│  (Expo)      │               │  HTTP + WS      │
├──────────────┤               │  Port 3000      │
│  飞书机器人  │◄─ 长连接 ────►│                 │
└──────────────┘               └────────┬────────┘
                                        │
                                 Cloudflare Tunnel
                                   (可选)
```

---

## 快速开始

### 安装

**方式一：下载预编译二进制（推荐）**

从 [GitHub Releases](https://github.com/codeweiz/remote-desktop-control/releases) 下载，解压即可运行：

```bash
# macOS（Apple Silicon 和 Intel 均支持，Intel 通过 Rosetta 2 兼容）
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-darwin-arm64.tar.gz | tar xz

# Linux x64
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-linux-x64.tar.gz | tar xz

# Linux ARM64（树莓派等）
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-linux-arm64.tar.gz | tar xz
```

**方式二：从源码安装（需要 Node.js 22+）**

```bash
git clone https://github.com/codeweiz/remote-desktop-control.git
cd remote-desktop-control
make install   # 安装依赖
make start     # 构建并启动
```

### 启动服务

```bash
# 基本启动（通过 Web 面板创建和管理会话）
./rtb start

# 启动时自动创建一个 claude 会话
./rtb start claude

# 指定端口
./rtb start --port 8080

# 启动并开启 Cloudflare Tunnel（可从公网访问）
./rtb start --tunnel
```

启动后终端会输出访问地址（含认证 Token）和一个 **QR Code**：

```
Remote Terminal Bridge v2 started!
  Web Panel:    http://192.168.1.100:3000?token=xxx
  Local:        http://localhost:3000?token=xxx

  Mobile: scan QR code to connect
  ██████████████████
  ██ QR Code here ██
  ██████████████████
```

- **Web 访问**：在浏览器中打开输出的 URL 即可使用终端
- **手机访问**：用 RTB App 扫描终端中的 QR Code 即可自动连接

### 配置管理

配置存储在 `~/.rtb/config.json`，支持交互式配置：

```bash
./rtb config          # 查看当前配置
./rtb config set      # 交互式配置（端口、隧道、飞书等）
```

也可以通过命令行参数或环境变量覆盖：

| 命令行参数 | 环境变量 | 说明 |
|-----------|---------|------|
| `--port <port>` | - | Web 终端端口（默认 3000） |
| `--tunnel` | - | 启用 Cloudflare Tunnel |
| `--feishu-app-id <id>` | `FEISHU_APP_ID` | 飞书应用 App ID |
| `--feishu-app-secret <s>` | `FEISHU_APP_SECRET` | 飞书应用 App Secret |
| `--feishu-chat-id <id>` | `FEISHU_CHAT_ID` | 飞书群聊 ID（可选） |

优先级：命令行参数 > 环境变量 > 配置文件。

### Cloudflare Tunnel（远程访问）

**快速隧道**（临时域名，无需配置）：
```bash
./rtb start --tunnel
# 会自动分配一个 xxx.trycloudflare.com 域名
```

**命名隧道**（固定域名，需提前配置）：
```bash
cloudflared login
cloudflared tunnel create rtb
cloudflared tunnel route dns rtb rtb.your-domain.com
./rtb tunnel setup rtb rtb.your-domain.com
./rtb start --tunnel
```

### 移动端 App

RTB 提供基于 Expo (React Native) 的 iOS/Android 客户端。

**连接方式**：服务器启动后终端会显示 QR Code，在 App 连接页扫码即可自动连接。也支持手动输入地址和 Token。

**下载**：Android APK 可在 [GitHub Releases](https://github.com/codeweiz/remote-desktop-control/releases) 下载。

### REST API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/sessions` | 获取所有会话列表 |
| POST | `/api/sessions` | 创建会话 `{name, command, args}` |
| DELETE | `/api/sessions/:id` | 删除会话 |
| GET | `/api/sessions/buffer?id=` | 获取会话终端输出缓冲区 |
| GET | `/api/notifications` | 获取通知设置 |
| POST | `/api/notifications` | 更新通知设置 `{channel, enabled}` |
| GET | `/api/settings` | 获取服务器设置 |

---

## 开发指南

### 环境要求

- Node.js 22+
- npm
- Xcode（iOS 开发）/ Android Studio（Android 开发）

### 项目结构

```
src/                    # 服务端 TypeScript 源码
  cli.ts                # 命令行入口
  server.ts             # HTTP 服务 + REST API
  ws-server.ts          # WebSocket 服务，终端数据双向传输
  session-manager.ts    # 会话生命周期管理
  pty-manager.ts        # node-pty 伪终端封装
  feishu.ts             # 飞书机器人集成（长连接模式）
  tunnel.ts             # Cloudflare Tunnel 集成
  notification.ts       # 通知管理（浏览器推送 + 飞书）
  auth.ts               # Token 认证
  config.ts             # 配置文件管理（~/.rtb/config.json）
web/                    # Web 终端面板（单文件 SPA，无构建步骤）
mobile/                 # Expo React Native 移动端 App
build-binary.mjs        # 跨平台二进制构建脚本（esbuild + Node.js SEA）
```

### 服务端开发

```bash
make install            # 安装依赖
make dev                # 开发模式启动（tsx 直接运行）
make build              # 编译 TypeScript 到 dist/
make start              # 编译并启动服务
make start-tunnel       # 编译并启动（开启 Cloudflare Tunnel）
make start-claude       # 编译并启动（自动创建 claude 会话）
make test               # 运行测试（vitest）
```

### 移动端开发

```bash
make mobile-install     # 安装移动端依赖
make mobile-start       # 启动 Expo 开发服务器
make mobile-ios         # 在 iOS 模拟器上运行
make mobile-android     # 在 Android 模拟器上运行
```

### 构建独立二进制

使用 [Node.js SEA](https://nodejs.org/api/single-executable-applications.html) 将服务端打包为单个可执行文件：

```bash
make build-binary       # 产物在 release/rtb
```

### 发布新版本

推送版本 tag 即可触发自动构建：

```bash
npm version patch       # 更新版本号
git push --follow-tags  # 触发 GitHub Actions 自动构建和发布
```

---

## 技术栈

| 组件 | 技术 |
|------|------|
| 服务端 | Node.js, TypeScript, node-pty, ws |
| Web 面板 | xterm.js（单文件 SPA） |
| 移动端 | Expo, React Native, WebView, expo-camera |
| 隧道 | Cloudflare Tunnel (cloudflared) |
| 即时通讯 | 飞书开放平台 SDK（长连接模式） |
| 测试 | Vitest |
| 打包 | esbuild + Node.js SEA |
| CI/CD | GitHub Actions |

## 许可证

MIT
