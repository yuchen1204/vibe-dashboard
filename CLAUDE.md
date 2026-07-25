# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Vibe Dashboard — 本地单用户的 AI 编程管理工具。后端 Rust (Axum + SQLx + SQLite)，前端 Vite + React + TypeScript + shadcn/ui。

目前已完成 **L1（基础设施层）+ L2（任务层）**，后续还有 L3（执行层，git worktree + coding agent）、L4（编排层，LLM 编排）、L5（审查层）。

## Build & Run

### Backend (port 8787)

```powershell
cd backend
cargo run -p api
```

**环境变量**: `VIBE_DB_PATH` (默认 `%APPDATA%\vibe-dashboard\data.db`), `VIBE_HTTP_PORT` (默认 8787), `VIBE_LOG_LEVEL` (默认 info)。`.env` 文件自动加载。

### Frontend (port 5173)

```powershell
cd frontend
npm install
npm run dev        # Vite dev server, 代理 /api 和 /ws 到后端
```

浏览器打开 http://localhost:5173

### Tests

```powershell
cd backend
cargo test                    # 全部测试（含 repo 单测 + API handler 集成测试）
cargo test -p api             # 仅 API crate
cargo test -p tasks           # 仅 tasks crate
cargo test -p tasks -- target_test   # 单个测试文件
cargo test -- workspace_target_todo_roundtrip  # 单个测试（按名称匹配）
```

```powershell
cd frontend
npm run typecheck
npm run lint
```

### Lint & SQLx

```powershell
cd backend
cargo clippy --all-targets -- -D warnings
cargo sqlx prepare --workspace   # 改 SQL 后必须跑，更新 .sqlx/ 离线缓存
```

## Architecture (5-Layer Design)

```
┌───────────────────────────────────────────────────────────────┐
│  Frontend: Vite + React 19 + TS 6 + shadcn/ui + Tailwind     │
│  TanStack Query (server state) + Zustand (UI state)           │
│  react-router-dom (路由), WebSocket (实时通道)                 │
└──────────────┬───────────────────────────────────────────────┘
               │ HTTP + WS
┌──────────────┴───────────────────────────────────────────────┐
│  Rust Backend (Axum + tokio)                                 │
│  L5 审查层  (review agent loop)          — 未实现            │
│  L4 编排层  (LLM 编排)                    — 未实现            │
│  L3 执行层  (git worktree + coding agent) — 未实现            │
│  L2 任务层  (Workspace/Target/To-Do CRUD) — 已实现            │
│  L1 基础设施  (HTTP server + WS hub + SQLite + 配置 + 日志)   │
└──────────────┬───────────────────────────────────────────────┘
               │
          ┌────┴────┐
          │ SQLite  │  (WAL 模式, foreign_keys ON)
          └─────────┘
```

### Crate Dependency Chain

```
shared  ←  db  ←  api
         ←  tasks  ←  api
```

- **`shared`**: 跨 crate 共享的错误类型 (`AppError`, `AppResult`), logging 初始化
- **`db`**: SQLite 连接池初始化 (`init_pool`, WAL 模式, foreign_keys ON, 5 连接上限), migration 执行 (`run_migrations`, `sqlx::migrate!("./migrations")`)
- **`tasks`**: 纯数据层，**不依赖 axum**。含 models (Workspace/Target/Todo + DTO + 状态枚举) 和 repo 函数 (所有 CRUD 操作，签名 `fn(pool, input) -> Result<_, AppError>`)。L3+ 可直接复用。
- **`api`**: 二进制 + 库 crate。Axum 路由 + handler。`AppState` 持有 `db: SqlitePool`, `hub: Arc<Hub>`, `config: Arc<Config>`, `started_at: DateTime<Utc>`。`ApiError` 是 `shared::AppError` 的薄包装，实现 `IntoResponse`。
- **`tasks` 测试依赖 `db`**（跑 migration 建临时库）

### Key Architectural Decisions

- **`tasks` crate 不依赖 axum**: L3/L4/L5 的 agent 代码可直接 import `tasks::repo` 读写任务状态，无需经过 HTTP
- **`AppError` 下沉到 `shared`**: 所有 crate 共享同一错误类型。`api` 侧用 `ApiError(pub shared::AppError)` 包装 + `From` 转换 + `IntoResponse`（孤儿规则绕行）
- **时间戳存 RFC3339 字符串**（`String` 类型），避免 chrono + sqlx FromRow 解析复杂度
- **ID 用 UUID v4 字符串**，Rust 侧 `Uuid::new_v4().to_string()`
- **SQLx 离线编译**: `.cargo/config.toml` 设 `SQLX_OFFLINE=true`，改 SQL 后手动跑 `cargo sqlx prepare --workspace` 更新 `.sqlx/` 缓存
- **L2 无 WebSocket 推送**: 单用户场景下 mutation invalidation 足够，WS 推送留到 L3（后台 agent 才需要 out-of-band 通知）
- **L2 无拖拽排序**: 看板用状态下拉菜单改状态，`sort_order` 字段已入库但无重排 UI

### Database Schema

两个 migration 文件在 `backend/crates/db/migrations/`:

- `0001_init.sql`: `schema_meta` 表（版本号 + 创建时间）
- `0002_tasks.sql`: `workspaces` → `targets` → `todos` 三层，`ON DELETE CASCADE`，status CHECK 约束，索引

### API Routes (L2 已完成)

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/health` | 健康检查 (status, version, uptime) |
| GET | `/api/path-suggest?q=` | 路径自动补全（Windows 文件系统） |
| GET/POST | `/api/workspaces` | 列表/创建工作区 |
| GET/PUT/DELETE | `/api/workspaces/:id` | 单个工作区 CRUD（GET 返回 target_count + todo_count） |
| GET/POST | `/api/workspaces/:wid/targets` | 列表/创建 target |
| GET/PUT/DELETE | `/api/targets/:id` | 单个 target CRUD |
| GET | `/api/workspaces/:wid/todos` | 跨 target 的全量 todo（看板数据源） |
| GET/POST | `/api/targets/:tid/todos` | 列表/创建 todo |
| GET/PUT/DELETE | `/api/todos/:id` | 单个 todo CRUD |
| GET | `/ws` | WebSocket 升级（hello/ping/pong 心跳） |

### Frontend Structure

```
src/
├── App.tsx                    # BrowserRouter + QueryClientProvider + Sidebar + 路由
├── pages/
│   ├── WorkspacesPage.tsx     # 工作区列表（卡片网格 + 新建对话框）
│   └── WorkspaceViewPage.tsx  # 工作区视图（target 侧栏 + 看板）
├── components/
│   ├── layout/Sidebar.tsx     # 导航 + 底部 health/ws 状态圆点
│   ├── workspace/             # WorkspaceCard, CreateWorkspaceDialog, PathInput
│   ├── target/                # TargetList, CreateTargetDialog
│   ├── board/                 # Board, BoardColumn, TodoCard, TodoDialog
│   └── ui/                    # shadcn/ui 组件 (button, card, dialog, select, input, ...)
├── hooks/
│   ├── useWorkspaces.ts       # TanStack Query hooks: list/create/update/delete
│   ├── useTargets.ts
│   ├── useTodos.ts
│   └── useGlobalStatus.ts     # health 探活 + WS 连接管理
├── lib/
│   ├── api.ts                 # fetch 封装 (getJson/postJson/putJson/del), 路径建议
│   ├── ws.ts                  # WebSocket 客户端 (指数退避重连, 最大 5 次)
│   ├── query.ts               # QueryClient 配置 (staleTime 30s, retry 1)
│   └── utils.ts               # cn() 工具函数
├── stores/
│   └── ui.ts                  # Zustand: wsStatus, connectionId, pingPongLatency
└── types/
    └── api.ts                 # 所有 API 类型定义 (Workspace, Target, Todo, DTOs, WS 消息)
```

### Testing Patterns

- **每个测试创建独立临时 SQLite 文件**（不用 `sqlite::memory:`，多连接池下各连接内存隔离会有问题）
- 模式：`init_pool` → `run_migrations` → 执行 repo 操作 → 断言（`TempDir` 自动清理）
- API handler 测试：`tower::ServiceExt::oneshot` 对 router 发请求，验证状态码 + 响应体
- Config 测试用 `#[serial]` + `clear_env()` 避免环境变量污染
- WS hub 测试：注册/注销/发送/广播/心跳

### Config & Environment

- 后端配置通过环境变量（`VIBE_*` 前缀），`.env` 文件在 `backend/` 目录
- SQLx 编译期配置在 `backend/.cargo/config.toml`（`DATABASE_URL`, `SQLX_OFFLINE`）
- 前端 Vite 代理配置在 `vite.config.ts`（`/api` → `:8787`, `/ws` → `ws://:8787`）