# RTB — Remote Terminal Bridge

远程终端桥接工具。在服务器上启动 RTB，即可通过 Web 浏览器、手机 App 或飞书机器人随时随地访问和管理终端会话。支持多会话管理、Cloudflare Tunnel 远程访问、飞书通知推送。

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

---

## 使用指南（用户）

如果你只想使用 RTB，不需要修改源码，请看这一节。

### 安装

**方式一：下载预编译二进制（推荐）**

从 [GitHub Releases](https://github.com/codeweiz/remote-desktop-control/releases) 下载对应平台的文件，解压即可运行，无需安装 Node.js：

```bash
# macOS Apple Silicon (M1/M2/M3/M4)
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-darwin-arm64.tar.gz | tar xz

# macOS Intel
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-darwin-x64.tar.gz | tar xz

# Linux x64
curl -fsSL https://github.com/codeweiz/remote-desktop-control/releases/latest/download/rtb-linux-x64.tar.gz | tar xz

# Linux ARM64 (树莓派等)
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

启动后终端会输出访问地址（含认证 token）和一个 **QR Code**：

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

RTB 的配置存储在 `~/.rtb/config.json`，支持交互式配置：

```bash
./rtb config          # 查看当前配置
./rtb config set      # 交互式配置（端口、隧道、飞书等）
```

也可以通过命令行参数或环境变量覆盖配置：

| 命令行参数 | 环境变量 | 说明 |
|-----------|---------|------|
| `--port <port>` | - | Web 终端端口（默认 3000） |
| `--tunnel` | - | 启用 Cloudflare Tunnel |
| `--feishu-app-id <id>` | `FEISHU_APP_ID` | 飞书应用 App ID |
| `--feishu-app-secret <s>` | `FEISHU_APP_SECRET` | 飞书应用 App Secret |
| `--feishu-chat-id <id>` | `FEISHU_CHAT_ID` | 飞书群聊 ID（可选） |

优先级：命令行参数 > 环境变量 > 配置文件。

### Cloudflare Tunnel（远程访问）

支持两种模式：

**快速隧道**（临时域名，无需配置）：
```bash
./rtb start --tunnel
# 会自动分配一个 xxx.trycloudflare.com 域名
```

**命名隧道**（固定域名，需提前配置）：
```bash
# 1. 登录 Cloudflare
cloudflared login

# 2. 创建隧道并绑定域名
cloudflared tunnel create rtb
cloudflared tunnel route dns rtb rtb.your-domain.com

# 3. 保存隧道配置到 RTB
./rtb tunnel setup rtb rtb.your-domain.com

# 4. 启动
./rtb start --tunnel
```

### 移动端 App

RTB 提供基于 Expo (React Native) 的 iOS/Android 客户端。

**连接方式**：服务器启动后终端会显示 QR Code，在 App 连接页点击「Scan QR Code」扫码即可自动连接。也支持手动输入地址和 Token。

### REST API

RTB 提供 HTTP API，可用于与其他工具集成：

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

## 开发指南（开发者）

如果你要修改 RTB 源码或构建移动端 App，请看这一节。

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
make dev                # 开发模式启动（tsx 直接运行，修改后需手动重启）
make build              # 编译 TypeScript 到 dist/
make start              # 编译并启动服务
make start-tunnel       # 编译并启动（开启 Cloudflare Tunnel）
make start-claude       # 编译并启动（自动创建 claude 会话）
make test               # 运行测试（vitest）
make test-watch         # 监听模式运行测试
```

### 配置相关

```bash
make config             # 查看当前配置
make config-set         # 交互式配置
make tunnel-setup NAME=rtb HOST=rtb.example.com  # 配置命名隧道
```

### 移动端开发

```bash
make mobile-install     # 安装移动端依赖
make mobile-start       # 启动 Expo 开发服务器
make mobile-ios         # 在 iOS 模拟器上运行（开发模式）
make mobile-android     # 在 Android 模拟器上运行（开发模式）
make mobile-web         # 在浏览器中运行
```

### 移动端打包

```bash
# 本地打包（直接安装到设备，推荐）
make mobile-release-ios       # 构建 Release 版 iOS App 并安装到 iPhone
make mobile-release-android   # 构建 Release 版 Android App

# EAS 云构建（需要 Expo 账号）
make mobile-build-dev         # 开发版（模拟器用）
make mobile-build-preview     # 预览版（生成 APK）
make mobile-build-prod        # 正式版
```

### 构建独立二进制

使用 Node.js SEA (Single Executable Application) 将服务端打包为单个可执行文件：

```bash
make build-binary       # 为当前平台构建 release/rtb
```

构建产物在 `release/rtb`，可直接拷贝到目标机器运行，无需安装 Node.js。

### CI/CD

项目配置了 GitHub Actions：

- **CI**（`.github/workflows/ci.yml`）：push/PR 到 main 分支时自动运行构建和测试
- **Release**（`.github/workflows/release.yml`）：推送 `v*` tag 或手动触发时，自动为 4 个平台构建二进制并发布到 GitHub Releases

**发布新版本：**

```bash
# 更新版本号
npm version patch       # 0.1.0 -> 0.1.1

# 推送代码和 tag，触发自动构建和发布
git push --follow-tags
```

### 清理

```bash
make clean              # 删除 dist/ 和 mobile/.expo 构建产物
```

---

## 技术栈

| 组件 | 技术 |
|------|------|
| 服务端 | Node.js, TypeScript, node-pty, ws |
| Web 面板 | xterm.js（单文件 SPA，无构建步骤） |
| 移动端 | Expo, React Native, WebView, expo-camera |
| 隧道 | Cloudflare Tunnel (cloudflared) |
| 即时通讯 | 飞书开放平台 SDK（长连接模式） |
| 测试 | Vitest |
| 打包 | esbuild + Node.js SEA |
| CI/CD | GitHub Actions |
