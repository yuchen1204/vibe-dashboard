# Vibe Dashboard L3 执行层 Implementation Plan

**Goal:** 在 L2 任务层上构建执行层：git worktree 管理 + coding agent 子进程 + 实时日志推送 + 前端执行按钮/日志面板。

**Architecture:** 新增 `backend/crates/execution`（模型 + repo + worktree 操作 + agent 管理）。api crate 编排后台执行任务，通过 WS 推送实时输出。前端 `TodoCard` 加执行按钮，底部弹出日志面板。

**Spec:** `docs/superpowers/specs/2026-07-25-vibe-dashboard-l3-execution-design.md`

## Global Constraints

- 沿用 L1/L2 全部约束。
- 所有 repo 函数用 `sqlx::query_as!` / `sqlx::query!` 宏。改 SQL 后 `cargo sqlx prepare --workspace`。
- `SQLX_OFFLINE=true` 保持开启。
- 每个 task 结束前必须 `cargo fmt --all` + `cargo clippy --all-targets -- -D warnings`（后端）或 `npm run typecheck`（前端）通过。
- 测试用独立临时文件库。
- 平台：Windows（PowerShell）。

---

## Task 1: execution crate 骨架 + migration 0003

**Files:**
- Modify: `backend/Cargo.toml`（members 加 execution）
- Create: `backend/crates/execution/Cargo.toml`
- Create: `backend/crates/execution/src/lib.rs`
- Create: `backend/crates/execution/src/models.rs`
- Create: `backend/crates/execution/src/repo.rs`（占位）
- Create: `backend/crates/execution/src/worktree.rs`（占位）
- Create: `backend/crates/execution/src/agent.rs`（占位）
- Create: `backend/crates/db/migrations/0003_execution.sql`

## Task 2: execution repo 实现

**Files:**
- Modify: `backend/crates/execution/src/repo.rs`
- Create: `backend/crates/execution/tests/worktree_test.rs`
- Create: `backend/crates/execution/tests/job_test.rs`

## Task 3: git worktree 操作 + agent 子进程管理

**Files:**
- Modify: `backend/crates/execution/src/worktree.rs`
- Modify: `backend/crates/execution/src/agent.rs`
- Create: `backend/crates/execution/tests/worktree_integration_test.rs`

## Task 4: API 路由 + WS 消息扩展 + 后台执行编排

**Files:**
- Modify: `backend/crates/api/Cargo.toml`（加 execution 依赖）
- Create: `backend/crates/api/src/routes/execution.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`
- Modify: `backend/crates/api/src/lib.rs`（注册路由 + 执行状态字段）
- Modify: `backend/crates/api/src/ws/message.rs`（追加 job_output/job_status）
- Modify: `backend/crates/api/src/state.rs`（可选）
- Modify: `backend/crates/api/src/ws/session.rs`（处理新消息类型）

## Task 5: 前端类型 + hooks + 组件

**Files:**
- Modify: `frontend/src/types/api.ts`（追加 Worktree/ExecutionJob 类型 + WS 消息）
- Create: `frontend/src/hooks/useExecution.ts`
- Create: `frontend/src/components/execution/ExecuteButton.tsx`
- Create: `frontend/src/components/execution/JobLogPanel.tsx`
- Modify: `frontend/src/components/board/TodoCard.tsx`（加执行按钮）
- Modify: `frontend/src/pages/WorkspaceViewPage.tsx`（加日志面板）
- Modify: `frontend/src/hooks/useGlobalStatus.ts`（订阅 job 消息）

## Task 6: 端到端联调 + 质量门禁