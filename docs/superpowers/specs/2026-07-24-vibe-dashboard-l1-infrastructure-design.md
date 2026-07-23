# Vibe Dashboard - 整体设计概览与 L1 基础设施层设计

**日期**: 2026-07-24
**状态**: 设计已确认，待 review

## 背景与目标

Vibe Dashboard 是一个本地单用户的 AI 编程管理工具。用户打开 Web UI 选择 workspace 进入后，可以在其中创建 Target（里程碑）和 To-Do（可执行任务），并通过编排 Agent 调动本地的 codex / opencode / claude code 等 coding agent 在 git worktree 中执行任务，由独立的 review agent 审查产出并反馈循环，最终产出可由用户决定合并的分支。

## 技术栈

| 层 | 选型 |
|---|---|
| 后端语言 | Rust |
| 后端 Web 框架 | Axum + tokio |
| 数据库 | SQLite（WAL 模式） |
| ORM / DB 访问 | SQLx（`query!` 宏 + 编译期校验） |
| 实时通信 | WebSocket over JSON |
| 前端构建 | Vite |
| 前端框架 | React + TypeScript |
| 状态管理 | TanStack Query（服务端状态）+ Zustand（UI 状态） |
| UI 组件 | shadcn/ui + Tailwind |
| LLM 接入 | 后端直调 OpenAI 兼容 API（编排 Agent 大脑） |
| 部署模型 | 本地单用户 |

## 整体系统架构（5 层）

系统按依赖顺序拆分为 5 个独立子项目，每个子项目单独走 spec -> plan -> 实现流程：

```
┌──────────────────────────────────────────────────────────────────────┐
│  Vibe Dashboard（本地单用户）                                          │
│                                                                       │
│  ┌─────────────── React + Vite (TS) + shadcn/ui + Tailwind ───────┐  │
│  │  Workspace 选择页 -> 工作区视图（看板 + 编排 Agent 聊天侧边栏）   │  │
│  │  实时数据：TanStack Query (REST) + WebSocket (JSON 推送)        │  │
│  └────────────────────────────┬────────────────────────────────────┘  │
│                               │ HTTP + WS                            │
│  ┌────────────────────────────┴────────────────────────────────────┐ │
│  │  Rust 后端 (Axum + tokio)                                        │ │
│  │                                                                  │ │
│  │  L5 审查循环层   review agent loop  ──┐                          │ │
│  │  L4 编排层       LLM 驱动的编排 Agent  │ 调度                     │ │
│  │  L3 执行层       git worktree + coding agent (子进程)             │ │
│  │  L2 任务层       Workspace/Target/To-Do CRUD                      │ │
│  │  L1 基础设施     HTTP server + WS hub + SQLite + 配置 + 日志     │ │
│  └────────────────────────────┬────────────────────────────────────┘ │
│                               │                                      │
│                        ┌──────┴───────┐                              │
│                        │  SQLite 文件  │                              │
│                        └──────────────┘                              │
└──────────────────────────────────────────────────────────────────────┘
```

**分层职责与依赖**（从下到上，上层依赖下层）：

| 层 | 名称 | 职责 | 依赖 |
|---|---|---|---|
| L1 | 基础设施 | HTTP/WS 服务器骨架、SQLite 连接池、配置加载、结构化日志、统一错误处理、AppState。**不含任何业务实体。** | 无 |
| L2 | 任务层 | Workspace / Target / To-Do 的 schema、CRUD REST API、看板 UI | L1 |
| L3 | 执行层 | git worktree 管理、启动 coding agent 子进程、stdout/stderr 实时流回前端 | L1, L2 |
| L4 | 编排层 | OpenAI 兼容 LLM 调用封装、编排 Agent 会话循环、聊天侧边栏 WS 通道、调度 L3 的 coding agent | L1, L2, L3 |
| L5 | 审查层 | review agent、git diff 抓取、评论存储、反馈 coding agent 的审查循环 | L1, L2, L3, L4 |

**推进策略**：按层切，A 层完成后才做 B 层。每层完成后用户都能用上对应功能，不白做。本次规范只覆盖 **L1 基础设施层**。L2-L5 每层后续单独写 spec。

## L1 基础设施层 - 详细设计

### L1 范围

L1 只做地基，不做任何业务功能。完成 L1 后系统具备：能启动的 Rust 后端、能启动的 Vite 前端、可用的 SQLite、可用的 WebSocket 双向通道。前端能 ping 后端、能建立 WS 连接收发 hello/ping/pong。

### L1 验收标准

- [ ] `cargo run -p api` 启动后端，`curl localhost:8787/api/health` 返回 200 + JSON `{status:"ok", version, uptime}`
- [ ] `npm run dev` 启动前端，浏览器打开能看到后端 health 信息和 WS 连接状态
- [ ] 点 ping 按钮，能在前端看到 pong 和往返延迟
- [ ] 杀掉后端，前端显示断线 banner；重启后端，前端自动重连（指数退避，最大 5 次后停止）
- [ ] `cargo sqlx prepare` 生成 `.sqlx/` 缓存成功（CI 无需连库可编译）
- [ ] `cargo test` 通过（含 DB 初始化单元测试）
- [ ] `cargo clippy --all-targets` 无 warning
- [ ] `npm run build` 前端构建成功
- [ ] `npm run typecheck` 通过

### 项目目录结构

```
vibe-dashboard/
├── backend/                         # Rust workspace
│   ├── Cargo.toml                   # workspace 根（members = api, db, shared）
│   ├── crates/
│   │   ├── api/                     # 二进制 crate：axum server 入口
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   │       ├── main.rs          # tokio main + 启动 server
│   │   │       ├── config.rs        # 配置加载（env + 默认值）
│   │   │       ├── state.rs         # AppState（DB pool、WS hub 等）
│   │   │       ├── error.rs         # 统一错误类型 + IntoResponse
│   │   │       ├── routes/
│   │   │       │   ├── mod.rs
│   │   │       │   ├── health.rs    # GET /api/health
│   │   │       │   └── ws.rs        # GET /ws (升级 WebSocket)
│   │   │       └── ws/
│   │   │           ├── mod.rs
│   │   │           └── hub.rs       # WS 连接管理 + 广播
│   │   ├── db/                      # 库 crate：数据库访问
│   │   │   ├── Cargo.toml
│   │   │   ├── migrations/          # SQLx 迁移文件
│   │   │   │   └── 0001_init.sql
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       └── pool.rs          # 连接池初始化
│   │   └── shared/                  # 库 crate：通用工具
│   │       ├── Cargo.toml
│   │       └── src/
│   │           ├── lib.rs
│   │           └── logging.rs       # tracing 初始化
├── frontend/                        # Vite + React + TS
│   ├── package.json
│   ├── vite.config.ts               # 代理 /api 和 /ws 到后端
│   ├── tsconfig.json
│   ├── index.html
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── lib/
│       │   ├── api.ts               # REST 客户端（fetch 封装）
│       │   ├── ws.ts                # WebSocket 客户端 + 重连
│       │   └── query.ts            # TanStack Query client
│       ├── stores/
│       │   └── ui.ts                # Zustand UI 状态
│       └── pages/
│           └── HomePage.tsx         # 占位首页（ping 后端）
├── docs/
│   └── superpowers/
│       └── specs/                   # 本设计文档放这里
├── .gitignore
└── README.md
```

**关键决策**：
- Rust 用 **cargo workspace + 多 crate**（`api` / `db` / `shared`），为后续 L2-L5 加 crate 留位置。L2 加 `crates/tasks`、L3 加 `crates/execution`、L4 加 `crates/orchestrator`、L5 加 `crates/review`。
- 前端 Vite dev server 通过 `server.proxy` 把 `/api` 和 `/ws` 转发到 `localhost:8787`（后端端口），开发期前后端独立热更。
- L1 只暴露两个端点：`GET /api/health`（健康检查，验证后端活着）和 `GET /ws`（WebSocket 升级，验证双向通道）。业务端点 L2 才加。

### 数据库与配置

**SQLite 文件位置**：`<用户配置目录>/vibe-dashboard/data.db`
- Windows: `%APPDATA%\vibe-dashboard\data.db`
- 可被环境变量 `VIBE_DB_PATH` 覆盖，便于测试和迁移。

**L1 的 schema（migrations/0001_init.sql）只建元数据表**，业务表 L2 才加：

```sql
-- 只存 schema 版本和实例元信息，业务表后续 migration 加
CREATE TABLE IF NOT EXISTS schema_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO schema_meta(key, value) VALUES('schema_version', '1')
    ON CONFLICT(key) DO UPDATE SET value = excluded.value;
INSERT INTO schema_meta(key, value) VALUES('created_at', strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    ON CONFLICT(key) DO NOTHING;
```

**SQLx 使用方式**：
- 用 `sqlx::query!` 宏 + `sqlx-cli` 在编译期校验 SQL，需要 `DATABASE_URL` 指向一个 dev 数据库。
- 提供 `cargo sqlx prepare`（CI 用）生成 `.sqlx/` 缓存，避免 CI 必须连库。
- migration 用 `sqlx::migrate!` 宏运行时自动执行。

**配置加载（config.rs）**：
- 优先级：环境变量 > 默认值（L1 暂不加配置文件，保持最小）。
- 配置项：
  - `VIBE_DB_PATH`（默认 `%APPDATA%\vibe-dashboard\data.db`）
  - `VIBE_HTTP_PORT`（默认 `8787`）
  - `VIBE_LOG_LEVEL`（默认 `info`）
- 配置项以 `Config` struct 暴露，放进 `AppState`。

**日志**：
- 用 `tracing` + `tracing-subscriber`，JSON 格式输出到 stdout（本地看也够用，未来要落文件再加）。
- HTTP 请求日志用 `tower-http::trace::TraceLayer`。

**初始化流程**（`main.rs`）：
1. 加载配置
2. 初始化 tracing
3. 创建 SQLite 连接池（`SqlitePoolOptions`，开启 `WAL` 模式和 `PRAGMA foreign_keys=ON`）
4. 运行 migrations
5. 构建 `AppState`
6. 启动 axum server，监听 `127.0.0.1:{port}`
7. 优雅退出（监听 SIGINT/SIGTERM，tokio `shutdown_signal`）

### HTTP 与 WebSocket 设计

#### HTTP 端点（L1 只有一个）

| 方法 | 路径 | 用途 |
|------|------|------|
| `GET` | `/api/health` | 返回 `{status:"ok", version, uptime}`。前端启动时探活，验证后端可达。 |

> `/ws` 走 HTTP upgrade，不算 REST。

#### WebSocket 通道设计

**连接**：`GET /ws` 升级为 WebSocket。每个浏览器 tab 一个连接。

**消息格式**（JSON，双向）：

```jsonc
// 服务端 -> 客户端（连接建立后立即发）
{ "type": "hello", "payload": { "connection_id": "uuid", "server_time": "ISO8601" } }
// 客户端 -> 服务端（L1 只支持 ping）
{ "type": "ping" }
// 服务端 -> 客户端
{ "type": "pong", "payload": { "server_time": "ISO8601" } }
```

**L1 只实现 hello/ping/pong**。真实业务消息（任务状态变更、agent 日志、编排对话等）在 L2-L5 各自添加 `type`。

#### WS Hub 架构（`ws/hub.rs`）

```text
Hub（Arc<Hub> 放进 AppState，持有 mpsc::Sender 和 DashMap<ConnId, Sender>）
  │
  ├── Client #1 (uuid) ── 收到消息 -> 转发到 Hub 处理任务
  ├── Client #2 (uuid)
  └── ...

Hub 暴露 publish(topic, msg) 方法：上层（L2+ 的业务逻辑）调它广播给所有或指定连接。
```

- 每个连接持有自己的 `mpsc::Sender<Message>`，Hub 用 `DashMap<ConnId, Sender>` 管理所有连接。
- Hub 是 `Arc<Hub>` 放进 `AppState`，后续层通过它广播。
- L1 的 Hub 内部循环只处理 ping/pong 和连接管理；后续层加新的 message handler。
- 心跳：服务端每 30s 发 WebSocket `ping` 帧，10s 内没收到 `pong` 则断开（防僵尸连接）。

#### 统一错误处理（`error.rs`）

```rust
pub enum AppError {
    Database(sqlx::Error),
    Internal(String),
    BadRequest(String),
    NotFound(String),
    // ... L2+ 加更多变体
}
impl IntoResponse for AppError {
    // 映射到 HTTP 状态码 + JSON body {error: "..."}
}
```

所有 handler 返回 `Result<Json<T>, AppError>`，业务错误用 `?` 传播，自动转 HTTP 响应。

#### AppState

```rust
pub struct AppState {
    pub db: SqlitePool,
    pub hub: Arc<Hub>,
    pub config: Arc<Config>,
}
```

放进 axum 的 `State`，所有 handler 共享。后续层加字段（如 LLM client、worktree manager）。

### 前端骨架与集成

#### 前端骨架结构

```text
frontend/src/
├── main.tsx              # React 挂载 + QueryClientProvider
├── App.tsx               # 路由（L1 只有一个占位页）+ 全局 layout
├── lib/
│   ├── api.ts            # fetch 封装：request<T>(method, path) -> Promise<T>
│   │                     #   - 自动拼 /api 前缀
│   │                     #   - 统一错误处理（抛 AppError）
│   │                     #   - getJson/postJson/putJson/del 辅助
│   ├── ws.ts             # WebSocket 客户端类
│   │                     #   - 自动重连（指数退避，最大 5 次后停止）
│   │                     #   - subscribe(type, handler) 订阅消息
│   │                     #   - send(type, payload) 发送
│   │                     #   - 连接状态（connecting/open/closed）
│   └── query.ts          # queryClient 配置 + 默认 queryFn=api
├── stores/
│   └── ui.ts             # Zustand store：全局 UI 状态（L1 存 ws 连接状态）
├── components/
│   ├── layout/           # 全局布局组件（sidebar/header 占位）
│   └── ui/               # shadcn/ui 生成的组件
└── pages/
    └── HomePage.tsx      # 占位页：显示后端 health 状态 + WS 连接状态
```

#### L1 前端功能（最小可验证）

1. **启动时**：调 `GET /api/health`，展示后端版本、uptime。
2. **WebSocket**：连接 `/ws`，收到 `hello` 显示连接 ID；点按钮发 `ping`，收到 `pong` 显示往返延迟。
3. **UI**：shadcn/ui 的 Card 展示上面信息，左边一个占位 Sidebar（"Workspaces" 文字，L2 才填实）。
4. **状态**：TanStack Query 管 health 查询；Zustand 存 WS 连接状态；WS 断线时显示 banner。

#### Vite 配置（`vite.config.ts`）

```ts
server: {
  port: 5173,
  proxy: {
    '/api': 'http://127.0.0.1:8787',
    '/ws':  { target: 'ws://127.0.0.1:8787', ws: true },
  }
}
```

#### 启动方式

- **开发**：两个终端，`cargo run -p api`（后端 8787）+ `npm run dev`（前端 5173）。前端通过 proxy 访问后端。
- **生产**（L1 暂不做但留接口）：后端 `axum::Router` 把 `/` 静态文件服务指向 `frontend/dist`，单端口。L1 先保证 dev 模式可用。

### 测试策略

- **后端**：
  - `db` crate：用临时 SQLite 文件做单元测试，验证连接池初始化、migration 执行成功、schema_meta 表存在。
  - `api` crate：用 `axum::test` + `tower::ServiceExt::oneshot` 测 `/api/health` 返回 200 + 正确字段。
  - WS hub：单元测试验证连接加入/移除、broadcast 广播到所有连接。
- **前端**：L1 不写测试，依赖 `npm run typecheck` 和手动验收。

### 错误处理策略

- 后端：`AppError` 统一错误类型，`?` 传播，`IntoResponse` 转 HTTP 响应。日志记录所有 5xx。
- 前端：fetch 封装统一抛错，TanStack Query 自动处理重试和错误状态展示。WS 断线 banner。

### 非目标（L1 明确不做）

- 任何业务实体（Workspace/Target/To-Do 的 schema 或 API）—— L2 做
- git worktree 管理、coding agent 启动 —— L3 做
- LLM 调用、编排 Agent —— L4 做
- review agent、diff 审查 —— L5 做
- 用户认证、多用户、远程部署
- 前端路由（L1 只一个占位页，路由 L2 加）
- 生产构建打包（L1 只保证 dev 模式可用）
- 配置文件（L1 只用环境变量）
