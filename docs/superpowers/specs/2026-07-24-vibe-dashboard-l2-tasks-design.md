# Vibe Dashboard - L2 任务层设计

**日期**: 2026-07-24
**状态**: 设计已确认，待写 plan

## 背景与目标

L1 基础设施层已完成（可启动的 Rust 后端 + Vite 前端 + SQLite + WebSocket hello/ping/pong）。L2 在其上构建**任务层**：让用户能管理工作区（Workspace）、里程碑（Target）、可执行任务（To-Do），并通过看板 UI 组织任务。

完成 L2 后，用户可以：创建 workspace → 进入 workspace → 创建 target → 在 target 下创建 todo → 在看板上按状态（待办/进行中/已完成/阻塞）查看和移动 todo。此时系统是一个**可用的本地任务看板**，尚不涉及 git worktree 和 coding agent（L3+）。

## 范围

**L2 做**：
- 三个业务实体的数据库 schema（migration `0002_tasks.sql`）
- `crates/tasks` crate：领域模型 + repository（用 `query!` / `query_as!` 宏，编译期 SQL 校验）
- `api` crate 的 REST CRUD 路由
- 前端：workspace 选择页 + workspace 看板视图（react-router 路由、TanStack Query 数据层、shadcn 看板组件）

**L2 不做**（明确推迟）：
- WebSocket 推送任务变更事件 —— 单用户本地工具，CRUD 由用户自己触发，TanStack Query 的 mutation + invalidation 即可让 UI 即时更新。WS 推送留到 L3（后台 agent 进程改变 todo 状态时才真正需要 out-of-band 推送）。
- 看板拖拽排序 —— L2 用卡片上的状态下拉/按钮改状态。拖拽交互复杂度高，推迟到后续迭代。`sort_order` 字段先入库但 L2 不暴露重排 UI。
- workspace.path 的 git 仓库校验 —— L2 只存路径字符串，是否是有效 git 仓库由 L3 验证。
- 分页 —— 单用户、数据量小，列表直接全量返回。
- 用户认证、多用户。

## 实体关系

```
Workspace (1) ──< (*) Target (1) ──< (*) To-Do
```

- **Workspace**：顶层容器，对应一个本地目录（通常是 git 仓库根路径）。用户打开 app 先选 workspace。
- **Target**：workspace 内的里程碑，用于把一组相关 todo 归到一起（如"重构数据库层"）。有自身状态（规划中/进行中/已完成/已归档）。
- **To-Do**：target 下的可执行任务，看板卡片的最小单位。状态即看板列。

严格层级：每个 todo 属于且仅属于一个 target；每个 target 属于且仅属于一个 workspace。删除 workspace 级联删除其下所有 target 和 todo（FK `ON DELETE CASCADE`）。

## 数据模型

新增 migration `backend/crates/db/migrations/0002_tasks.sql`（集中放在 db crate，沿用 L1 的 `sqlx::migrate!("./migrations")`）：

```sql
CREATE TABLE workspaces (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    path       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE targets (
    id          TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'planned'
                CHECK (status IN ('planned', 'active', 'done', 'archived')),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE todos (
    id          TEXT PRIMARY KEY,
    target_id   TEXT NOT NULL,
    title       TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    status      TEXT NOT NULL DEFAULT 'todo'
                CHECK (status IN ('todo', 'doing', 'done', 'blocked')),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    FOREIGN KEY (target_id) REFERENCES targets(id) ON DELETE CASCADE
);

CREATE INDEX idx_targets_workspace ON targets(workspace_id);
CREATE INDEX idx_todos_target      ON todos(target_id);
CREATE INDEX idx_todos_status      ON todos(status);
```

**关键约定**：
- `id`：UUID v4，在 Rust 侧用 `Uuid::new_v4().to_string()` 生成，存为 TEXT。
- 时间戳：ISO8601 字符串（`Utc::now().to_rfc3339()`），存为 TEXT，模型字段类型用 `String`（避免 chrono `FromRow` 解析复杂度，前端按需展示）。
- `foreign_keys=ON`：L1 的 pool 已设置 `PRAGMA foreign_keys`，级联删除生效。
- `status`：DB 层 CHECK 约束 + Rust 层枚举校验双重防御。

## REST API 设计

所有路径以 `/api` 前缀。JSON 字段 snake_case，与 Rust serde 一致。列表全量返回（无分页）。成功创建/更新返回实体本身；删除返回 `204 No Content`。

### Workspaces

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/workspaces` | 列出所有 workspace（按 `updated_at` desc） |
| `POST` | `/api/workspaces` | 创建。body: `{ name: string, path: string }` |
| `GET` | `/api/workspaces/:id` | 取单个（含 target/todo 计数） |
| `PUT` | `/api/workspaces/:id` | 更新。body: `{ name?: string, path?: string }` |
| `DELETE` | `/api/workspaces/:id` | 删除（级联） |

`Workspace` 响应：`{ id, name, path, created_at, updated_at }`。
`GET /:id` 额外返回 `{ target_count, todo_count }`。

### Targets

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/workspaces/:wid/targets` | 列出 workspace 下 target（按 `sort_order`, `created_at`） |
| `POST` | `/api/workspaces/:wid/targets` | 创建。body: `{ title: string, description?: string }` |
| `GET` | `/api/targets/:id` | 取单个 |
| `PUT` | `/api/targets/:id` | 更新。body: `{ title?, description?, status?, sort_order? }` |
| `DELETE` | `/api/targets/:id` | 删除（级联其下 todo） |

`Target` 响应：`{ id, workspace_id, title, description, status, sort_order, created_at, updated_at }`。

### To-Dos

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/workspaces/:wid/todos` | 列出 workspace 下所有 todo（跨 target，看板用） |
| `GET` | `/api/targets/:tid/todos` | 列出单个 target 下 todo |
| `POST` | `/api/targets/:tid/todos` | 创建。body: `{ title: string, description?: string }` |
| `GET` | `/api/todos/:id` | 取单个 |
| `PUT` | `/api/todos/:id` | 更新。body: `{ title?, description?, status?, sort_order? }` |
| `DELETE` | `/api/todos/:id` | 删除 |

`Todo` 响应：`{ id, target_id, title, description, status, sort_order, created_at, updated_at }`。

**错误映射**（复用 L1 `AppError`）：
- 实体不存在 → `404 NotFound`
- body 字段非法（空 title、非法 status）→ `400 BadRequest`
- DB 约束冲突 / sqlx 错误 → `500 Internal`（记日志）

## Crate 结构

新增 `backend/crates/tasks`，在 workspace `members` 加入 `"crates/tasks"`。

```
backend/crates/tasks/
├── Cargo.toml
└── src/
    ├── lib.rs          # re-export
    ├── models.rs       # Workspace/Target/Todo + Create*/Update* DTO + 状态枚举
    └── repo.rs         # repository 函数（query_as! / query!），均接 &SqlitePool
```

**职责边界**：
- `tasks` crate 只含**模型 + 数据访问**，不依赖 axum。repo 函数签名形如 `pub async fn create_todo(pool: &SqlitePool, target_id: &str, input: CreateTodo) -> Result<Todo, AppError>`。
- `api` crate 新增 `routes/tasks.rs`（+ 在 `routes/mod.rs` 注册），handler 从 `AppState.db` 取 pool，调 `tasks::repo::*`，结果用 `?` 透传 `AppError`。
- 这样 L3/L4/L5 可直接复用 `tasks::repo` 读写任务状态（如 L3 agent 执行完更新 todo 状态），不被 web 框架耦合。

**依赖**：
- `tasks` 依赖：`sqlx`、`uuid`、`chrono`、`serde`、`thiserror`，以及 `shared`（错误类型复用 —— 见下）。
- `tasks` dev-依赖：`db`（测试里跑 migration）、`tokio`（`#[tokio::test]`）。
- `api` 新增依赖 `tasks`（path）。

**错误类型归属决策**：把 L1 的 `AppError`（及 `AppResult`）从 `api::error` **下沉到 `shared::error`**。`shared` crate 不依赖 axum，因此 `IntoResponse` impl 留在 `api` 侧（`impl IntoResponse for shared::AppError`），`api/error.rs` 改为 re-export 或保留兼容别名。`tasks`、未来的 execution/orchestrator/review crate 都依赖 `shared` 拿到同一错误类型，避免层层转换。这是 L1 的小重构（移动类型 + 调整 import），代价可接受。

## SQLx 工作流

L2 是首个用 `query!` 宏的层，需建立编译期校验工作流。L1 的 `.cargo/config.toml` 已设 `DATABASE_URL=sqlite:./dev.db` 与 `SQLX_OFFLINE=true`。

**决策：保留 `SQLX_OFFLINE=true`**（不改 L1 配置），采用离线缓存工作流：
1. 写/改 migration 或 repo SQL 后，先确保 `dev.db` 已应用最新 migration：在 `backend/` 跑一次 `cargo run -p api`（main.rs 启动时 `db::run_migrations` 会建表），Ctrl+C 停掉。
2. 跑 `cargo sqlx prepare --workspace`，更新 `backend/.sqlx/` 缓存并 commit。
3. 之后 `cargo build` / `cargo test` 用 `.sqlx/` 离线缓存校验，无需连库。

> 这样 CI 设 `SQLX_OFFLINE=true` 即可无库编译。代价是每次改 SQL 多一步 `prepare`，对单人项目可接受。

**测试用的临时库**：repo 单测不能依赖 `dev.db`，每个测试用独立临时文件库：`format!("sqlite:{}/vibe_test_{}.db", env::temp_dir().to_string_lossy(), Uuid::new_v4())`，`init_pool` + `run_migrations` 后跑 repo 函数。不用 `sqlite::memory:`（多连接池下各连接内存隔离，会踩坑）。

## 前端设计

### 路由

L1 无路由（单 HomePage）。L2 引入 `react-router-dom` v6：

| 路径 | 组件 | 说明 |
|------|------|------|
| `/` | `WorkspacesPage` | workspace 列表 + 新建（取代 L1 的 HomePage） |
| `/workspaces/:wid` | `WorkspaceViewPage` | 看板视图 |

`App.tsx` 改为 `<BrowserRouter>` 包 `<Routes>`。**L1 的 HomePage 不再单独保留**；其 health/ws 状态信息降级为 Sidebar 底部的状态指示器（见下）。

### Sidebar 改造

Sidebar 同时承担导航和全局状态指示：

- **主体**：workspace 导航列表（来自 `useWorkspaces()`），当前 `:wid` 高亮。顶部"所有工作区"入口指回 `/`。
- **底部状态指示器**：两个圆点 + 文字标签，放在 Sidebar 最下方，分别表示后端 health 与 WebSocket 连接状态：
  - health 圆点：health 查询成功（`status === "ok"`）= 绿色，查询失败/不可达 = 红色
  - ws 圆点：`wsStatus === "open"` = 绿色，其他（connecting/closed）= 红色
  - 圆点用一个小 `<span>` + Tailwind 颜色类（`bg-green-500` / `bg-red-500`）实现，无需额外组件
  - 数据来源：health 走 TanStack Query（`/api/health`，refetchInterval 保持），ws 状态走 L1 的 Zustand `useUiStore.wsStatus`。两者均在 Sidebar 组件内订阅，HomePage 里的订阅逻辑迁移到此。

### 看板默认视图

看板**默认显示 workspace 下全部 todo**（跨所有 target），不强制选 target。Target 侧栏提供"按 target 过滤"作为可选缩小范围，`selectedTargetId === null` = 全部。这样用户进入 workspace 即看到完整看板，无需额外操作。

### 页面与组件

```
frontend/src/
├── App.tsx                       # Router + QueryClientProvider + layout
├── pages/
│   ├── WorkspacesPage.tsx        # workspace 卡片网格 + 新建 dialog（取代 L1 HomePage）
│   └── WorkspaceViewPage.tsx     # target 侧栏 + 看板
├── components/
│   ├── layout/Sidebar.tsx        # 改造：workspace 导航 + 底部 health/ws 状态圆点
│   ├── workspace/
│   │   ├── WorkspaceCard.tsx
│   │   └── CreateWorkspaceDialog.tsx
│   ├── target/
│   │   ├── TargetList.tsx        # 侧栏 target 列表 + 选中 + 新建（含"全部"选项）
│   │   └── CreateTargetDialog.tsx
│   └── board/
│       ├── Board.tsx             # 4 列看板（默认显示全部 todo）
│       ├── BoardColumn.tsx       # 单列（按 status）
│       ├── TodoCard.tsx          # 卡片（标题 + target badge + 状态/编辑/删除）
│       └── TodoDialog.tsx        # 新建/编辑 todo 表单
├── lib/api.ts                    # 复用 L1 的 request 封装
├── hooks/
│   ├── useWorkspaces.ts          # TanStack Query: list/create/update/delete
│   ├── useTargets.ts
│   └── useTodos.ts
└── types/api.ts                  # 扩展 Workspace/Target/Todo 类型
```

> L1 的 `pages/HomePage.tsx` 删除，其 health/ws 订阅与展示逻辑迁入 `Sidebar.tsx`（降级为底部圆点指示器）。

### 新增 shadcn 组件

在 L1 的 card/button/badge 基础上，用 `npx shadcn@latest add` 加：`dialog`、`input`、`textarea`、`label`、`select`、`dropdown-menu`。

### 数据流与状态

- **服务端状态**：全走 TanStack Query。
  - `useWorkspaces()` → `GET /api/workspaces`
  - `useTargets(wid)` → `GET /api/workspaces/:wid/targets`
  - `useTodos(wid)` → `GET /api/workspaces/:wid/todos`（看板数据源）
  - 各 `useMutation` 完成后 `queryClient.invalidateQueries(['workspaces', wid, ...])`
- **本地 UI 状态**：`WorkspaceViewPage` 用 `useState` 存 `selectedTargetId`（默认 `null` = 全部）。看板按 `selectedTargetId` 过滤 todo（`null` 即不过滤，显示全部），再按 status 分组到 4 列。
- **Zustand**：L1 的 `ui` store（ws 状态）不动，L2 不新增全局 store。
- **实时性**：mutation 后 invalidation 触发 refetch，UI 即时更新。无 WS 推送（见范围）。

### 看板交互

- 4 列：待办（todo）/ 进行中（doing）/ 已完成（done）/ 阻塞（blocked）。
- 卡片：标题、所属 target 的 badge、描述摘要。点卡片打开 `TodoDialog` 编辑；卡片上有状态下拉快速改列。
- 新建 todo：列头"＋"按钮或顶部按钮，打开 `TodoDialog`，选 target + 填标题。
- 不做拖拽（推迟）。改状态靠下拉/编辑表单。

## 测试策略

**后端**：
- `tasks` crate repo 单测：临时文件库 + migration，覆盖 CRUD、级联删除、status CHECK 约束拒绝非法值、按 workspace/target 列出。每个测试独立库避免互相污染。
- `api` crate handler 测试：`tower::ServiceExt::oneshot` 对 router 发请求，验证 200/201/204/404/400 + 响应体字段。复用临时库建 `AppState`。
- `cargo sqlx prepare --workspace` 生成 `.sqlx/`，保证 `SQLX_OFFLINE=true` 下编译通过。

**前端**：L2 不写单测，依赖 `npm run typecheck` + 手动验收。

## 验收标准

- [ ] `POST/GET/PUT/DELETE /api/workspaces` 全部可用，删除 workspace 级联清掉其 target 和 todo
- [ ] `POST/GET/PUT/DELETE /api/targets`、`/api/todos` 同上
- [ ] `GET /api/workspaces/:wid/todos` 返回跨 target 的全量 todo 供看板
- [ ] 非法 status 被 DB CHECK 或 Rust 校验拒绝（400/500）
- [ ] `cargo sqlx prepare --workspace` 成功生成 `.sqlx/`；`SQLX_OFFLINE=true cargo build` 通过
- [ ] `cargo test`（含 tasks repo 测试 + api handler 测试）通过
- [ ] `cargo clippy --all-targets -- -D warnings` 无 warning
- [ ] 前端：`/` 看到 workspace 列表，新建并进入 → 看到 target 侧栏 + 4 列看板 → 新建 todo 出现在待办列 → 改状态后卡片移到对应列 → 删除 workspace 后回到列表且数据消失
- [ ] `npm run typecheck` + `npm run build` 通过

## 非目标（再次明确）

- WS 推送任务事件（L3）
- 看板拖拽排序（后续迭代）
- workspace.path 的 git 仓库存在性/有效性校验（L3）
- 分页、搜索、过滤 beyond target 选中
- 用户认证、多用户、远程部署
- 生产构建单端口打包（沿用 L1，只保 dev）
