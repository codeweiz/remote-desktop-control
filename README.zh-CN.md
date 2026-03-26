# RTB 2.0 -- Remote Terminal Bridge

**[English](./README.md)**

基于 Rust 构建的高性能远程终端桥接工具，提供 Web 终端访问、AI Agent 集成和插件化架构。

## 功能特性

- **远程终端访问** -- 通过浏览器和移动端访问完整终端会话
- **AI Agent 管理** -- 通过 ACP (Agent Communication Protocol) 连接和管理 AI 代理
- **插件架构** -- 可扩展的 IM 集成 (飞书、Telegram、Discord) 和隧道提供商
- **智能通知** -- 3 层检测引擎 (关键词、正则、语义) 实现智能告警
- **任务池** -- 支持优先级管理的自动调度任务队列
- **现代化 UI** -- 深色/浅色主题、命令面板 (Ctrl+K)、快捷键
- **单文件分发** -- 内嵌前端资源的自包含二进制文件

## 架构概览

```
+------------------+     +------------------+
|   CLI (clap)     |     |   Tauri 桌面端   |
+--------+---------+     +--------+---------+
         |                        |
         +----------+-------------+
                    |
         +----------v-----------+
         |    Server (Axum)     |
         |  REST + WebSocket    |
         +----------+-----------+
                    |
         +----------v-----------+
         |        Core          |
         |  +------+ +-------+ |
         |  |事件  | | PTY   | |
         |  | 总线 | | 管理器| |
         |  +------+ +-------+ |
         |  +------+ +-------+ |
         |  |Agent | |会话   | |
         |  | ACP  | | 存储  | |
         |  +------+ +-------+ |
         |  +------+ +-------+ |
         |  |通知  | | 任务  | |
         |  |引擎  | |  池   | |
         |  +------+ +-------+ |
         +-----------+----------+
                     |
         +-----------v----------+
         |    Plugin Host       |
         |  IM / 隧道插件       |
         +----------------------+
```

## 快速开始

```bash
# 构建前端
cd web && npm install && npm run build && cd ..

# 构建 Rust 二进制文件
cargo build --release -p rtb-cli

# 运行
./target/release/rtb-cli
```

在浏览器中打开 `http://localhost:9399` 即可使用。

或使用 Makefile:

```bash
make dev      # 开发模式启动（自动编译插件）
make build    # 构建前端 + 插件 + 发布版二进制
make install  # 构建并安装到 /usr/local/bin/rtb
```

## CLI 用法

```bash
rtb                  # 启动 RTB 服务器 (前台运行)
rtb start -d         # 以守护进程方式启动
rtb stop             # 停止守护进程
rtb status           # 查看服务器状态
```

## 插件

### Cloudflare Tunnel

启动时自动通过 `cloudflared` 创建公网 URL（Quick Tunnel 模式，无需 Cloudflare 账号）。

```bash
brew install cloudflared   # 安装一次即可
make dev                   # Tunnel 随服务自动启动
```

### 飞书 IM

通过 WebSocket 长连接接收飞书消息，无需公网域名。配置凭证：

```bash
export FEISHU_APP_ID="your_app_id"
export FEISHU_APP_SECRET="your_app_secret"
make dev
```

飞书开放平台配置：
1. 创建企业自建应用
2. 事件订阅 → 订阅方式选择「使用长连接接收事件」
3. 添加事件：`im.message.receive_v1`

### IM 交互命令

在飞书中给机器人发消息即可与 AI Agent 交互：

| 命令 | 说明 |
|------|------|
| *(任意文本)* | 与当前 Agent 对话（无 Agent 时自动创建） |
| `/new [provider]` | 创建新 Agent（默认 `claude-code`） |
| `/list` | 列出所有 Agent（带序号） |
| `/switch N` | 切换到第 N 个 Agent |
| `/help` | 显示帮助信息 |

Agent 的输出（文本、工具调用、进度、错误）会自动转发到飞书聊天中。

## 技术栈

| 层       | 技术                                |
|----------|-------------------------------------|
| 后端     | Rust, Tokio, Axum, portable-pty     |
| 前端     | React 19, TypeScript, Tailwind CSS  |
| 终端     | xterm.js                            |
| 桌面端   | Tauri 2                             |
| 构建     | Cargo workspace, Vite               |

## 开发指南

```bash
# 终端 1: 启动 Rust 后端
cargo run -p rtb-cli

# 终端 2: 启动前端开发服务器
cd web && npm run dev
```

```bash
make help    # 查看所有可用目标
make test    # 运行所有测试
make clean   # 清理构建产物
```

## 项目结构

```
remote-desktop-control/
+-- crates/
|   +-- core/           # 核心库: 事件总线、PTY、会话、代理、通知、任务池
|   +-- server/         # Axum HTTP/WS 服务器、REST API、静态文件嵌入
|   +-- plugin-host/    # 插件管理器、IM 和隧道插件接口
|   +-- cli/            # CLI 入口 (clap)、守护进程生命周期
+-- web/
|   +-- src/
|   |   +-- components/ # React 组件 (Terminal, SessionList, AgentChat 等)
|   |   +-- hooks/      # 自定义 Hooks (useTerminal, useWebSocket, useTheme 等)
|   |   +-- lib/        # API 客户端、WebSocket 工具、类型定义
|   +-- index.html
|   +-- tailwind.config.js
|   +-- vite.config.ts
+-- docs/               # 设计规格和实现计划
+-- .github/            # CI/CD 工作流
+-- Cargo.toml          # 工作空间根配置
+-- Makefile            # 构建命令
+-- LICENSE             # MIT
```

## 开源许可

[MIT](LICENSE)
