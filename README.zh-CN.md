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
make build    # 构建前端 + 发布版二进制
make install  # 构建并安装到 /usr/local/bin/rtb
```

## CLI 用法

```bash
rtb                  # 启动 RTB 服务器 (前台运行)
rtb start -d         # 以守护进程方式启动
rtb stop             # 停止守护进程
rtb status           # 查看服务器状态

rtb session list     # 列出活跃的终端会话
rtb session new      # 创建新的终端会话
```

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

其他命令:

```bash
make test    # 运行所有测试 (Rust + 前端)
make lint    # 运行代码格式检查 + clippy
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
