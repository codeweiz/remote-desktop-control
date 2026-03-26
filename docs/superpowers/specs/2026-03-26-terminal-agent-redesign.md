# Terminal + Agent 系统重设计

> 日期：2026-03-26
> 状态：Reviewed
> 参考项目：[VibeAround](../../), [Mitto](../../)

## 前置条件

- **tmux >= 2.6** 必须安装在 PATH 中。服务端启动时检查，不满足则报错退出并给出安装指引。
- 不支持降级回裸 shell 模式——tmux 是硬依赖。
- Docker/容器环境需在镜像中安装 tmux。
- `agent-client-protocol` crate 需要验证 crates.io 上是否存在。如不存在，从 VibeAround 项目 vendor 或 fork。
- **升级说明**：所有现有会话在升级后失效，需要重新创建。

## 背景

当前项目存在四个核心问题：

1. **Agent 不可用** — 手写的 ACP 协议实现与真实 Agent CLI（Claude Code、Gemini 等）不兼容
2. **Terminal 卡顿** — base64+JSON 编码开销导致延迟，回车需按两次，光标位置错误
3. **Task Pool 功能不完整** — 无法删除任务，与 Agent 无联动
4. **Focus View UI 遮挡** — Agent 面板不存在时仍尝试打开 AgentDrawer，遮挡返回按钮

## 设计原则

- 不考虑向后兼容，当前是开发阶段，追求最优方案
- 学习 VibeAround 和 Mitto 的成熟做法，不闭门造车
- Agent 是 Terminal 的智能伙伴，共享同一个工作目录

## 整体架构

```
┌──────────────────────────────────────────────────┐
│ Browser (React 19 + xterm.js)                    │
│  ┌─ GridView ──┐  ┌─ FocusView ───────────────┐ │
│  │ SessionCards │  │ Terminal  │  AgentDrawer   │ │
│  │ TaskPool    │  │ (tmux)    │  (chat/tools)  │ │
│  └─────────────┘  └───────────┴────────────────┘ │
├──────────────────────────────────────────────────┤
│ WebSocket                                        │
│  Terminal: Binary frames (raw bytes)             │
│  Agent: JSON Text frames (ACP events)            │
│  Control: JSON Text frames (resize/keepalive)    │
├──────────────────────────────────────────────────┤
│ Axum Server                                      │
│  ├─ ws/terminal.rs   (PTY I/O bridge)           │
│  ├─ ws/agent.rs      (ACP event relay)          │
│  └─ api/             (REST: sessions, tasks)    │
├──────────────────────────────────────────────────┤
│ Core                                             │
│  ├─ pty/             (tmux-based sessions)      │
│  ├─ agent/           (ACP SDK + adapters)       │
│  ├─ task_pool/       (backlog + dispatcher)     │
│  └─ event_bus/       (control + data channels)  │
└──────────────────────────────────────────────────┘
```

---

## Phase 1：Terminal 系统重设计

### 1.1 tmux 作为 PTY 后端

替换当前裸 shell 方案，参考 VibeAround。

**tmux 命名约定**：`rtb-{session_id}`，避免与用户 tmux 会话冲突。

**会话创建**：
```
新建：bash -c "cd '/workspace' && exec tmux new-session -s 'rtb-{session_id}'"
重连：tmux attach -d -t 'rtb-{session_id}'
```

**环境变量**（tmux 内部继承）：
```
TERM=xterm-256color
COLORTERM=truecolor
```

**简化**：
- 去掉 `RingBuffer` + replay 机制 → tmux 自带 scrollback，重连时 attach 恢复
- 去掉 `convertEol` → tmux 处理终端语义
- 去掉 `coalesce_ms` 配置 → tmux 自己做输出缓冲
- 去掉 `replay_gap` / `replay_done` 协议 → 不需要了
- 会话持久化天然支持 → WebSocket 断了 tmux 还在

**会话生命周期**：

| 操作 | 行为 |
|------|------|
| 创建 | `tmux new-session -s 'rtb-{id}'` |
| WebSocket 断开 | tmux 会话保持（detach），等待重连 |
| WebSocket 重连 | `tmux attach -d -t 'rtb-{id}'`，tmux 重绘屏幕 |
| 用户删除会话 | `tmux kill-session -t 'rtb-{id}'` + 杀 PTY 子进程 |
| 服务端关闭 | 遍历所有 `rtb-*` 命名的 tmux 会话并 kill |
| 服务端启动 | 扫描残留的 `rtb-*` tmux 会话并清理（孤儿回收） |

**启动时孤儿回收**：
```rust
fn cleanup_orphan_tmux_sessions() {
    // tmux list-sessions -F '#{session_name}' | grep '^rtb-'
    // 对每个匹配的会话执行 tmux kill-session -t '{name}'
}
```

**PTY 读取线程**（简化后）：
```rust
let mut buf = [0u8; 4096];
loop {
    match reader.read(&mut buf) {
        Ok(0) => break,
        Ok(n) => {
            let chunk = &buf[..n];
            osc_responder.intercept(chunk);
            let _ = live_tx.send(Bytes::from(chunk.to_vec()));
        }
        Err(_) => break,
    }
}
```

### 1.2 WebSocket 协议重设计

**输出（服务端 → 客户端）**：
- PTY 输出：`Message::Binary(raw_bytes)` — 去掉 base64+JSON
- 控制消息：Text 帧 JSON，如 `{"type":"exit","code":0}`、`{"type":"status","status":"running"}`

**输入（客户端 → 服务端）**：
- PTY 输入：`ws.send(encoder.encode(data))` Binary 帧（避免与 JSON 控制消息冲突）
- 控制命令：Text 帧 JSON，如 `{"type":"resize","cols":120,"rows":40}`
- 设计理由：Binary/Text 帧类型天然区分输入和控制，不存在歧义（例如用户 echo 一个 JSON 字符串不会被误解析为 resize 命令）

**背压处理**（参考 Mitto）：
- 服务端发送时如果 buffer 满，等待 100ms
- 仍然满 → 主动关闭连接
- 客户端自动重连，tmux attach 恢复所有内容
- **防重连风暴**：tmux attach 重绘大屏幕可能再次触发背压。解决方案：重连后先发送 `tmux capture-pane -p`（仅可见区域），再切换到 live streaming

**Keepalive + 健康监控**（参考 Mitto）：
```
客户端每 10s：{"type":"keepalive","client_time":1234567890}
服务端响应：{"type":"keepalive_ack","server_time":1234567891}
```
tmux 模式下 seq 的作用简化为连接健康监控，不再用于数据完整性保证。

### 1.3 OSC 颜色拦截（参考 VibeAround）

在服务端 PTY 读取线程中拦截 OSC 10/11 查询，直接写回 PTY：

```rust
struct OscColorResponder {
    osc10_response: Vec<u8>,  // "\x1b]10;rgb:c8c8d8\x1b\\"
    osc11_response: Vec<u8>,  // "\x1b]11;rgb:0d0d0d\x1b\\"
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl OscColorResponder {
    fn intercept(&self, chunk: &[u8]) {
        // 检测 OSC 10;? 或 OSC 11;? 查询序列：
        //   \x1b]10;?\x1b\\  或  \x1b]10;?\x07  (OSC 10 query, ST 或 BEL 结尾)
        //   \x1b]11;?\x1b\\  或  \x1b]11;?\x07  (OSC 11 query)
        // 注意：只拦截查询（带 ?），不拦截 set-color 命令
        // 注意：OSC 序列可能跨越 read 边界，需用简单状态机缓冲不完整序列
        // 参考实现：VibeAround src/core/src/pty/runtime.rs:140-187
    }
}
```

效果：Neovim/Helix 等 TUI 程序启动时不需要等 WebSocket 往返。

**注意事项**：
- 只拦截 query（`?`），不拦截 set-color 命令
- 跨 read 边界的 OSC 序列需要状态机缓冲
- 写回响应时需与 PTY stdin writer 同步（共享 Mutex）

### 1.4 前端 xterm 优化

```typescript
const terminal = new Terminal({
  fontSize: isMobile ? 11 : 14,
  fontFamily: "'JetBrains Mono', ui-monospace, monospace",
  cursorBlink: true,
  cursorStyle: 'bar',
  scrollback: 0,     // tmux 管理 scrollback
  allowProposedApi: true,
})

// 渲染器降级链：WebGL → Canvas → DOM
try {
  const webgl = new WebglAddon()
  webgl.onContextLoss(() => {
    webgl.dispose()
    terminal.loadAddon(new CanvasAddon())
  })
  terminal.loadAddon(webgl)
} catch {
  try { terminal.loadAddon(new CanvasAddon()) } catch { /* DOM fallback */ }
}

// 输入：Binary 帧（与控制消息的 Text 帧区分）
terminal.onData(data => {
  if (ws.readyState === WebSocket.OPEN) {
    ws.send(new TextEncoder().encode(data))
  }
})

// 输出：Binary 帧直接写
ws.onmessage = (event) => {
  if (event.data instanceof ArrayBuffer) {
    terminal.write(new Uint8Array(event.data))
  } else if (typeof event.data === 'string') {
    handleControlMessage(JSON.parse(event.data))
  } else if (event.data instanceof Blob) {
    event.data.arrayBuffer().then(buf => terminal.write(new Uint8Array(buf)))
  }
}
```

### 1.5 移动端虚拟键盘（参考 VibeAround）

新增 `MobileInputBar` 组件：
- 快捷按钮：Ctrl / Esc / Tab / 方向键
- 检测虚拟键盘弹出（`useVisualViewportHeight` hook）
- 触摸滚动桥接（touch events → `term.scrollLines()`）
- 移动端禁用 xterm 直接 stdin（`disableStdin: isMobile`），通过 MobileInputBar 输入

### 1.6 重连逻辑

有了 tmux，重连变得简单：
1. WebSocket 断开 → 指数退避重连（保留现有逻辑）
2. 重连成功 → 服务端 `tmux attach -d -t '{session_id}'`
3. tmux 重绘整个屏幕 → 客户端自动恢复
4. 不需要 replay buffer、不需要 seq 回放

---

## Phase 2：Agent 系统 + Task Pool + 全局优化

### 2.1 ACP SDK 集成

替换手写 JSON-RPC，使用 `agent-client-protocol` crate（v0.9+）。

**去掉**：
- `crates/core/src/agent/acp_client.rs`（手写 JSON-RPC）
- `crates/core/src/agent/types.rs`（手写类型）

**新架构**：
```
AgentManager
  └─ AcpBackend（per agent）
       ├─ 独立线程 + 独立 tokio runtime（ACP futures 是 !Send）
       ├─ ClientSideConnection（ACP SDK）
       ├─ SharedAcpClientHandler（回调处理）
       └─ broadcast channel → WebSocket → 前端
```

线程模型（参考 VibeAround）：
```rust
std::thread::Builder::new()
    .name(format!("{}-acp", agent_kind))
    .spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
        rt.block_on(async {
            let local = tokio::task::LocalSet::new();
            local.run_until(acp_session_loop(cmd_rx, event_tx, ...)).await
        });
    })
```

ACP 协议流程：
```
Initialize() → 握手，获取 agent capabilities
NewSession(cwd, mcp_servers) → 获取 session_id
Prompt(session_id, content_blocks) → 发送用户消息
  ← SessionNotification 流式回调
```

权限处理：默认自动审批第一个选项。配置 `auto_approve_tools = false` 时，通过 WebSocket 推送审批请求到前端，用户手动确认。

**Agent WebSocket 协议**（`ws/agent.rs`）：

服务端 → 客户端（Text 帧 JSON）：
```json
// 文本回复
{"type":"text","seq":1,"content":"正在分析代码...","streaming":true}
// 思考过程
{"type":"thinking","seq":2,"content":"需要先检查 package.json..."}
// 工具调用
{"type":"tool_use","seq":3,"id":"tool_1","name":"Bash","input":{"command":"npm test"}}
// 工具结果
{"type":"tool_result","seq":4,"id":"tool_1","output":"All tests passed","is_error":false}
// 进度
{"type":"progress","seq":5,"message":"Using tool: Bash"}
// 一轮完成
{"type":"turn_complete","seq":6,"cost_usd":0.03}
// 状态变更
{"type":"status","status":"working"|"idle"|"waiting_approval"|"error"}
// 错误
{"type":"error","message":"...","severity":"permanent"|"transient","guidance":"..."}
```

客户端 → 服务端（Text 帧 JSON）：
```json
// 发送消息
{"type":"message","text":"请帮我重构这个函数"}
// 审批工具调用
{"type":"approve","tool_id":"tool_1"}
// 拒绝工具调用
{"type":"deny","tool_id":"tool_1","reason":"不要删除这个文件"}
// 取消当前操作
{"type":"cancel"}
```

**DataEvent 扩展**：
```rust
pub enum DataEvent {
    // Terminal 事件（保持不变）
    PtyOutput { seq: u64, data: Bytes },
    PtyExited { exit_code: i32 },
    // Agent 事件（重新设计）
    AgentText { seq: u64, content: String, streaming: bool },
    AgentThinking { seq: u64, content: String },
    AgentToolUse { seq: u64, id: String, name: String, input: Value },
    AgentToolResult { seq: u64, id: String, output: String, is_error: bool },
    AgentProgress { seq: u64, message: String },
    AgentTurnComplete { seq: u64, cost_usd: Option<f64> },
    AgentError { seq: u64, message: String, severity: ErrorSeverity, guidance: String },
}
```

### 2.2 多 Agent 适配

| Agent | 模式 | 启动命令 | 系统提示 |
|-------|------|---------|---------|
| Claude | 适配器桥接 | `claude --input-format stream-json --output-format stream-json` | `--system-prompt` flag |
| Gemini | 原生 ACP | `gemini --experimental-acp` | `GEMINI_SYSTEM_MD` 环境变量 |
| OpenCode | 原生 ACP | `opencode acp` | `AGENTS.md` 文件 |
| Codex | 原生 ACP | `npx @zed-industries/codex-acp` | `.codex/instructions.md` |

**Claude 适配器**（参考 VibeAround 的 `claude_sdk.rs` + `claude_acp.rs`）：
```
ClientSideConnection（ACP 端）
  ↕ duplex pipe
SharedAcpClientHandler（ACP server 端）
  ↕ 协议翻译
ClaudeAcpBridge
  ↕ NDJSON 解析
claude --input-format stream-json --output-format stream-json
```

Claude NDJSON 消息类型：`system`、`assistant`、`result`、`control_request`。
`can_use_tool` → 自动 `{"behavior": "allow"}`。

原生 ACP agents 直接 spawn 子进程，stdin/stdout 接 `ClientSideConnection`。

### 2.3 Agent-Terminal 绑定

Agent 和 Terminal 共享同一个工作目录，但各自有独立的 interface：

```
┌──────────────────────────────────────────────┐
│ Workspace: /home/user/project                │
│                                              │
│  ┌─ Terminal ──────┐  ┌─ Agent ────────────┐ │
│  │ tmux session    │  │ ACP subprocess     │ │
│  │ (用户交互)       │  │ (AI 工作)           │ │
│  │ 看到文件变化     │  │ 编辑文件/运行命令    │ │
│  └─────────────────┘  └────────────────────┘ │
│         ↑                       ↑            │
│         └── 同一个文件系统 ───────┘            │
└──────────────────────────────────────────────┘
```

- Agent 通过 ACP 工具（Bash、Edit、Write）修改文件
- 用户在 Terminal 实时看到变化
- AgentDrawer 显示结构化视图（thinking/text/tool_use/tool_result）
- 不是 Agent 往 tmux 打字，而是共享 workspace

会话创建：
```rust
let terminal = create_tmux_session(session_id, cwd);
let agent = create_agent(provider, model, cwd); // 同一个 cwd
agent.set_companion_terminal(terminal.id);
```

**会话关联**：通过 `SessionInfo.parent_id` 字段关联 Agent → Terminal。

**删除级联规则**：
| 操作 | 效果 |
|------|------|
| 删除 Terminal | 同时杀掉其 companion Agent |
| 删除 Agent | 不影响 Terminal |
| Agent 崩溃 | Terminal 不受影响，Agent 按重启策略恢复 |
| Terminal 退出 | Agent 保持运行（仍可在 cwd 工作），但标记为无 companion |

### 2.4 Task Pool 自动调度

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│  Task Pool  │────→│  Dispatcher  │────→│   Agent     │
│  (Backlog)  │     │  (调度器)     │     │  (ACP)      │
└─────────────┘     └──────────────┘     └─────────────┘
      ↑                                        │
      │         状态更新                         │
      └────────────────────────────────────────┘
```

**调度器架构**：
```rust
struct TaskDispatcher {
    task_pool: Arc<TaskPool>,
    agent_manager: Arc<AgentManager>,
    poll_interval: Duration,       // 如 30s
    max_concurrent: usize,         // 同时运行的 agent 进程数上限
}
```

调度逻辑：
```
每次 poll：
1. 统计当前 InProgress 任务数
2. 如果 < max_concurrent：
   a. 取优先级最高的 Pending 任务
   b. 查找空闲 Agent（status == Idle），或创建新 Agent
   c. 任务内容作为 prompt 发送给 Agent
   d. Pending → InProgress
3. Agent TurnComplete 事件到达时：
   a. 如果 config.auto_approve == true → InProgress → Completed
   b. 如果 config.auto_approve == false → InProgress → NeedsReview
4. 用户审核后 → Completed 或退回 Pending
```

**注意**：`max_concurrent` 是 N 个独立的 Agent **进程**，不是一个进程的 N 个并发 prompt。每个 ACP Agent 进程一次只处理一个 prompt。

**任务取消**：InProgress 任务可以被取消，触发 Agent 的 `cancel` 操作，任务回到 Pending。

任务生命周期：
```
Pending → InProgress → NeedsReview → Completed
   ↑           │            │
   └── 取消 ───┘            │
   └──────── 退回 ──────────┘
```

系统提示注入：
```
你正在处理一个任务：
标题：{task.title}
优先级：{task.priority}
描述：{task.description}

请在工作目录 {cwd} 中完成这个任务。完成后说明你做了什么。
```

### 2.5 错误分类 + 用户引导（参考 Mitto）

```rust
enum ErrorSeverity {
    Permanent,   // 配置错误，不会自动恢复
    Transient,   // 临时故障，可以重试
}

fn classify_error(stderr: &str, error: &str) -> (ErrorSeverity, String) {
    if contains("MODULE_NOT_FOUND") || contains("EACCES") || contains("permission denied") {
        (Permanent, "Agent 二进制不可用或权限不足，请检查安装".into())
    } else if contains("ENOENT") || contains("not found in PATH") {
        (Permanent, "找不到 agent 命令，请确认已安装并在 PATH 中".into())
    } else if contains("timeout") || contains("ECONNREFUSED") {
        (Transient, "网络超时，将自动重试".into())
    } else {
        (Transient, "未知错误，将尝试重启".into())
    }
}
```

重启策略：最多 3 次，指数退避 3s → 6s → 12s → 30s max。

### 2.6 辅助会话（参考 Mitto）

复用 ACP 进程内的轻量级辅助会话：

```rust
enum AuxPurpose {
    TitleGen,      // 为任务自动生成标题
    FollowUp,      // 生成后续操作建议
    Summarize,     // 总结 Agent 工作成果
}

let aux_session = process.new_session(cwd, AuxPurpose::TitleGen);
aux_session.prompt("请为以下内容生成一个简短的标题：...");
```

**错误处理**：辅助会话失败（rate limit、timeout、进程崩溃）时静默记录日志，功能优雅降级（如标题留空、不生成建议）。辅助会话的错误**不影响**主会话。

### 2.7 工作区配置 + 热加载

三层配置（参考 Mitto）：
```
~/.rtb/config.toml          ← 全局默认
/workspace/.rtb.toml         ← 工作区覆盖
运行时参数                     ← 最高优先级
```

工作区配置 `.rtb.toml`：
```toml
[agent]
default_provider = "claude"
auto_approve_tools = true
system_prompt = "你是一个专注于后端开发的助手"

[task_pool]
auto_start = true
max_concurrent = 2
```

**热加载范围**：

| 配置项 | 热加载 | 说明 |
|--------|--------|------|
| `agent.auto_approve_tools` | 是 | 立即生效于新的 prompt |
| `agent.system_prompt` | 是 | 下次 prompt 使用新提示 |
| `agent.default_provider` | 否 | 需重启，影响进程创建 |
| `task_pool.auto_start` | 是 | 立即开始/停止调度器 |
| `task_pool.max_concurrent` | 是 | 下次调度时生效 |
| `server.host/port` | 否 | 需重启 |

监控机制：使用 `notify` crate 监听文件变化，变更后重新 merge 三层配置。

### 2.8 UI 重构

**Focus View 修复**：
- Agent 不存在时不打开 AgentDrawer
- Agent 存在时：左侧 Terminal（tmux），右侧 AgentDrawer（chat）
- 可拖拽分割线调整比例

**AgentDrawer 重设计**：
```
┌─────────────────────────────────────────┐
│ [← Back]  Terminal: dev-server    [⚡]  │
├──────────────────────┬──────────────────┤
│                      │  Agent Chat      │
│   Terminal           │  ┌─thinking──┐   │
│   (tmux output)      │  │ 分析代码... │   │
│                      │  └───────────┘   │
│   $ npm run dev      │                  │
│   > ready on :3000   │  我已经启动了开发   │
│   $                  │  服务器，现在...    │
│                      │                  │
│                      │  ┌─tool_use──┐   │
│                      │  │ Bash: npm │   │
│                      │  │ test      │   │
│                      │  └───────────┘   │
│                      │                  │
│                      │  [输入消息...]     │
├──────────────────────┴──────────────────┤
│ Task Pool (2 pending)  [+ Add Task]     │
└─────────────────────────────────────────┘
```

**Grid View**：
- "New Agent" → 先选 provider，再选工作目录
- Session Card 显示 Agent 状态
- Task Pool 面板可折叠

**Task Pool UI**：
- 删除按钮正常工作
- 状态颜色标识
- 显示处理中的 Agent
- 完成后弹出 review 提示

---

## Agent 事件类型

```rust
pub enum AgentEvent {
    Text(String),                              // 回复文本
    Thinking(String),                          // 思考过程
    Progress(String),                          // 进度（如 "Using tool: Bash"）
    ToolUse { name: String, id: String, input: Value },  // 工具调用
    ToolResult { id: String, output: String, is_error: bool }, // 工具结果
    TurnComplete { cost_usd: Option<f64> },    // 一轮完成
    Error(String),                             // 错误
}
```

---

## 去掉的组件

| 组件 | 原因 |
|------|------|
| `RingBuffer` + replay 机制 | tmux 自带 scrollback |
| `replay_gap` / `replay_done` 协议 | tmux attach 恢复 |
| `coalesce_ms` 配置 | tmux 处理输出缓冲 |
| 手写 `AcpClient` + `types.rs` | 用 ACP SDK 替代 |
| base64 编码/解码 | Binary WebSocket 帧 |
| `convertEol` 配置 | tmux 处理终端语义 |

## 新增的组件

| 组件 | 来源 |
|------|------|
| tmux PTY 后端 | VibeAround |
| OSC 颜色拦截 | VibeAround |
| WebSocket 背压 + 强制重连 | Mitto |
| Keepalive 健康监控 | Mitto |
| WebGL → Canvas → DOM 降级链 | VibeAround |
| MobileInputBar 虚拟键盘 | VibeAround |
| ACP SDK 集成 | VibeAround (agent-client-protocol crate) |
| Claude 适配器桥接 | VibeAround (claude_sdk + claude_acp) |
| 原生 ACP agents (Gemini/OpenCode/Codex) | VibeAround |
| Task Pool 调度器 | 新设计 |
| 错误分类 + 用户引导 | Mitto |
| 辅助会话 (title-gen, follow-up) | Mitto |
| 工作区配置 .rtb.toml | Mitto (.mittorc) |
| 配置热加载 | Mitto |
