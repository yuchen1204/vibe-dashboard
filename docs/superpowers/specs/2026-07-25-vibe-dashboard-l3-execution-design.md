# Vibe Dashboard - L3 执行层设计

**日期**: 2026-07-25
**状态**: 设计稿

## 背景与目标

L2 任务层已完成，用户可管理工作区、Target、Todo，并通过看板 UI 组织任务。但 Todo 还只是文本卡片——不能真正"执行"。

L3 **执行层**让用户可以在 workspace 对应的 git 仓库中创建 worktree，启动 coding agent（Claude Code）子进程来执行 todo，并将 agent 的 stdout/stderr 实时流回前端。

完成 L3 后，用户可以：在看板中点击一个 todo → 选择"执行" → 系统自动创建 git worktree → 启动 coding agent → 前端实时看到 agent 输出 → agent 完成后 todo 标记为 done。

## 范围

**L3 做**：
- `crates/execution`：worktree 管理 + execution job 数据层
- migration `0003_execution.sql`：`worktrees` + `execution_jobs` 表
- Git worktree 操作（create/list/delete），通过 `git` CLI 子进程
- Coding agent 子进程管理（spawn、stream stdout/stderr、cancel）
- WebSocket 消息扩展：`job_output`、`job_status` 推送
- REST API：worktree CRUD + job 执行/取消/状态查询
- 前端：TodoCard 加"执行"按钮，执行日志面板，worktree 状态指示

**L3 不做**（明确推迟）：
- 多个 coding agent 并行执行（L3 一次只跑一个 job，队列执行）
- 编排层（L4 做）：LLM 决定做什么、调度多个 agent
- 审查层（L5 做）：review agent 审查产出
- 非 Claude Code 的 agent 支持（L3 只支持 Claude Code CLI，L4 抽象化）
- 终端模拟器 UI（L3 用简单的滚动日志面板，不做 xterm.js）
- 执行结果自动 diff 展示

## 实体关系

```
Workspace (1) ──< (*) Target (1) ──< (*) Todo
     │                                        │
     │                                        └──< (*) ExecutionJob
     │                                                  │
     └──< (*) Worktree ──< (1) ────────────────┘ (可选)
```

- **Worktree**：workspace 对应 git 仓库的一个 worktree（隔离分支），用于执行 coding 任务。一个 workspace 可有多個 worktree。
- **ExecutionJob**：一次 coding agent 执行记录，关联到一个 todo。记录 agent 类型、prompt、输出、状态、时间。

## 数据模型

新增 migration `backend/crates/db/migrations/0003_execution.sql`：

```sql
CREATE TABLE worktrees (
    id           TEXT NOT NULL PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    target_id    TEXT,                          -- 可选，关联的 target
    branch       TEXT NOT NULL,
    path         TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'active'
                 CHECK (status IN ('active', 'merged', 'abandoned')),
    created_at   TEXT NOT NULL,
    updated_at   TEXT NOT NULL,
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE execution_jobs (
    id          TEXT NOT NULL PRIMARY KEY,
    todo_id     TEXT NOT NULL,
    worktree_id TEXT,                          -- 可选，执行时创建或关联
    status      TEXT NOT NULL DEFAULT 'pending'
                CHECK (status IN ('pending', 'running', 'success', 'failed', 'cancelled')),
    agent_type  TEXT NOT NULL DEFAULT 'claude-code',
    prompt      TEXT NOT NULL,
    output      TEXT NOT NULL DEFAULT '',
    started_at  TEXT,
    finished_at TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    FOREIGN KEY (todo_id) REFERENCES todos(id) ON DELETE CASCADE
);

CREATE INDEX idx_worktrees_workspace ON worktrees(workspace_id);
CREATE INDEX idx_jobs_todo          ON execution_jobs(todo_id);
CREATE INDEX idx_jobs_status        ON execution_jobs(status);
```

## Crate 结构

新增 `backend/crates/execution`：

```
backend/crates/execution/
├── Cargo.toml
└── src/
    ├── lib.rs             # re-export
    ├── models.rs          # Worktree/ExecutionJob + DTO
    ├── repo.rs            # repository 函数（worktree CRUD + job CRUD）
    ├── worktree.rs        # git worktree 操作（CLI 包装）
    └── agent.rs           # coding agent 子进程管理
```

**依赖**：
- `execution` 依赖：`shared`（AppError）、`sqlx`、`uuid`、`chrono`、`serde`、`tokio`（process）
- `execution` dev-依赖：`db`（测试 migration）、`tempfile`
- `api` 新增依赖 `execution`（path）

**关键设计**：`execution` crate 不依赖 `tasks` crate。它通过 `todo_id` 字符串关联，不做跨 crate 实体引用。repo 层只处理自己的表。L4 编排层负责关联 tasks 和 execution。

## WebSocket 消息扩展

在 L2 的 `ClientMsg` / `ServerMsg` 基础上追加：

```jsonc
// 服务端 -> 客户端（job 输出流）
{ "type": "job_output", "payload": { "job_id": "uuid", "text": "compiling..." } }

// 服务端 -> 客户端（job 状态变更）
{ "type": "job_status", "payload": { "job_id": "uuid", "status": "running", "todo_id": "uuid" } }
```

## API 设计

### Worktrees

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/workspaces/:wid/worktrees` | 列出 workspace 下 worktree |
| `POST` | `/api/workspaces/:wid/worktrees` | 创建 worktree。body: `{ branch: string, target_id?: string }` |
| `DELETE` | `/api/worktrees/:id` | 删除 worktree（`git worktree remove`） |

### Execution Jobs

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/api/workspaces/:wid/jobs` | 列出 workspace 下所有 job |
| `POST` | `/api/todos/:tid/execute` | 开始执行 todo。body: `{ agent_type?: string }` |
| `GET` | `/api/jobs/:id` | 查 job 状态 + 输出 |
| `POST` | `/api/jobs/:id/cancel` | 取消运行中的 job |

**执行流程**：
1. `POST /api/todos/:tid/execute` → 创建 `execution_job`（status=pending）
2. 后端自动：创建/复用 worktree → 更新 job（status=running, worktree_id）→ spawn agent 子进程
3. 子进程 stdout/stderr → 逐行写入 `output` 字段 + WS 推送 `job_output`
4. 子进程退出 → 更新 job（status=success/failed, finished_at）→ WS 推送 `job_status` → 更新 todo status = done（若成功）

## 前端设计

### 新增/修改组件

```
frontend/src/
├── components/
│   └── execution/
│       ├── ExecuteButton.tsx     # TodoCard 上的"执行"按钮 + 状态指示
│       ├── JobLogPanel.tsx       # 执行日志面板（滚动输出）
│       └── WorktreeBadge.tsx     # worktree 分支标签
├── hooks/
│   └── useExecution.ts          # TanStack Query hooks: execute/cancel/jobs
├── pages/
│   └── WorkspaceViewPage.tsx     # 修改：加入 JobLogPanel
└── types/
    └── api.ts                    # 追加 Worktree/ExecutionJob 类型 + WS 消息
```

### 交互流程

1. **看板中的 TodoCard**：增加"执行"按钮（⚡图标）。点击 → 确认 → 调用 `POST /api/todos/:tid/execute`
2. **执行中**：按钮变 loading 态，底部弹出 JobLogPanel 显示实时输出
3. **执行完成**：todo 自动移到"已完成"列，日志面板显示最终结果
4. **执行失败**：todo 移到"阻塞"列，日志面板显示错误信息

### WebSocket 订阅

`useGlobalStatus` 或新的 `useJobStream` hook 订阅 WS 消息：
- `job_output` → 追加到日志缓冲区
- `job_status` → 更新 job 状态，触发 todo 列表 invalidation

## 测试策略

- `execution` crate repo 测试：临时文件库 + CRUD
- worktree 测试：需要 mock git 命令或使用临时 git 仓库（`git init` 临时目录）
- agent 测试：mock 子进程（用 `echo` 替代真实 agent）
- API handler 测试：`tower::ServiceExt::oneshot`
- 前端：手动验收

## 验收标准

- [ ] `cargo test` 全部通过（含 execution crate 测试）
- [ ] `cargo clippy --all-targets -- -D warnings` 无 warning
- [ ] `POST /api/todos/:tid/execute` 创建 job，执行完成后 todo 状态更新
- [ ] WebSocket 收到 `job_output` 和 `job_status` 消息
- [ ] 前端看板 TodoCard 有"执行"按钮，点击后看到实时日志
- [ ] `npm run typecheck` + `npm run build` 通过
- [ ] `cargo sqlx prepare --workspace` 通过
- [ ] `SQLX_OFFLINE=true cargo build` 通过