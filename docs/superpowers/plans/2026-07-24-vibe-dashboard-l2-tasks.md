# Vibe Dashboard L2 任务层 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 L1 基础设施上构建任务层：Workspace / Target / To-Do 三实体的 SQLite schema + `crates/tasks` repository（`query!` 宏编译期校验）+ REST CRUD 路由 + 前端看板 UI（react-router 路由、TanStack Query 数据层、4 列看板、Sidebar 底部 health/ws 状态圆点）。

**Architecture:** 新增 `backend/crates/tasks`（模型 + repo，不依赖 axum）。`AppError` 从 `api::error` 下沉到 `shared::error`，`tasks`/`api` 共享。前端引入 `react-router-dom`，`/` = WorkspacesPage，`/workspaces/:wid` = 看板视图。L1 HomePage 删除，health/ws 订阅迁入 `App.tsx` 的全局 hook，状态圆点显示在 Sidebar 底部。

**Spec:** `docs/superpowers/specs/2026-07-24-vibe-dashboard-l2-tasks-design.md`

**Tech Stack（沿用 L1 实际版本）:** Rust 1.97, Axum 0.7, SQLx 0.8 (SQLite), tokio, thiserror, uuid, chrono; React 19, TypeScript 6, Vite 8, TanStack Query 5, Zustand 5, shadcn/ui, Tailwind 3, react-router-dom 6, oxlint.

## Global Constraints

- 沿用 L1 全部约束：后端端口 8787、前端 dev 端口 5173、SQLite WAL + foreign_keys、tracing JSON、配置走环境变量、不加注释（除非用户要求）。
- 所有 repo 函数用 `sqlx::query_as!` / `sqlx::query!` 宏（编译期校验）。改 SQL 后必须 `cargo sqlx prepare --workspace` 更新 `.sqlx/` 缓存并 commit。
- `SQLX_OFFLINE=true` 保持开启（L1 已设于 `.cargo/config.toml`）。
- 每个 task 结束前必须 `cargo fmt --all` + `cargo clippy --all-targets -- -D warnings`（后端）或 `npm run typecheck`（前端）通过。
- 测试用独立临时文件库（`tempfile::tempdir` + `sqlite://...`），不用 `sqlite::memory:`（多连接池下内存库各连接隔离会踩坑，与 L1 `wal_mode_enabled` 测试同理）。
- 平台：Windows（PowerShell）。命令以 PowerShell 语法给出，cargo/npm 跨平台通用。

---

## Task 1: AppError 下沉到 shared crate

**Files:**
- Create: `backend/crates/shared/src/error.rs`
- Modify: `backend/crates/shared/src/lib.rs`
- Modify: `backend/crates/shared/Cargo.toml`
- Modify: `backend/crates/api/src/error.rs`
- Modify: `backend/crates/api/Cargo.toml`

**Interfaces:**
- Consumes: L1 `api::error::AppError`
- Produces: `shared::error::{AppError, AppResult}`，`api` 侧保留 `IntoResponse` impl + re-export

**Why first:** `tasks` crate 要返回 `AppError`，必须先让它从 shared 来，避免 api<->tasks 环依赖。这是后续所有 task 的前置。

- [ ] **Step 1: 给 shared crate 加依赖**

修改 `backend/crates/shared/Cargo.toml` 的 `[dependencies]`，加入：

```toml
sqlx.workspace = true
thiserror.workspace = true
serde_json.workspace = true
tracing.workspace = true
```

> `AppError` 含 `#[from] sqlx::Error` 和 `#[from] sqlx::migrate::MigrateError`，需要 sqlx。`IntoResponse` 里用 `serde_json::json!` 和 `tracing::error!`，但这两个在 shared 里不用（impl 留 api 侧）。此处只为 `#[from]` 加 sqlx/thiserror。serde_json/tracing 不加也可，但加上以备 shared 未来用。**最小化原则：只加 sqlx、thiserror。**

修正后 `shared/Cargo.toml` `[dependencies]`：

```toml
[dependencies]
tracing.workspace = true
tracing-subscriber.workspace = true
sqlx.workspace = true
thiserror.workspace = true
```

- [ ] **Step 2: 创建 `shared/src/error.rs`**

把 L1 `api/src/error.rs` 里的 `AppError` enum + `AppResult` type alias 搬过来，**不带** `IntoResponse` impl 和 tests（那些依赖 axum，留 api 侧）：

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("internal error: {0}")]
    Internal(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found: {0}")]
    NotFound(String),
}

pub type AppResult<T> = Result<T, AppError>;
```

> 注意：去掉 L1 的 `#[allow(dead_code)]`。L2 会实际使用 `Internal`/`BadRequest`/`NotFound` 变体，不再需要 dead_code allow。`Database`/`Migration` 也已被 `#[from]` 用到。

- [ ] **Step 3: 注册 shared 模块**

修改 `backend/crates/shared/src/lib.rs`：

```rust
pub mod error;
pub mod logging;

pub use error::{AppError, AppResult};
```

- [ ] **Step 4: 改写 `api/src/error.rs` 为 re-export + IntoResponse**

替换 `backend/crates/api/src/error.rs` 全文：

```rust
pub use shared::error::{AppError, AppResult};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::Database(_) | AppError::Migration(_) | AppError::Internal(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
        };
        tracing::error!(error = %self, status = %status, "request failed");
        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_error_maps_to_500() {
        let err = AppError::Database(sqlx::Error::PoolClosed);
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn bad_request_maps_to_400() {
        let err = AppError::BadRequest("missing field".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn not_found_maps_to_404() {
        let err = AppError::NotFound("widget".to_string());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
```

> `api` 仍依赖 `sqlx`（已有），`IntoResponse` impl 在 api crate（shared 不依赖 axum）。其余 crate 用 `shared::AppError`。

- [ ] **Step 5: 验证编译 + 测试**

```powershell
cd backend
cargo build -p api -p shared
cargo test -p api
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

Expected: 编译通过，api 3 个 error 测试仍通过（现在测的是 `shared::AppError` 的 IntoResponse），clippy 无 warning。

- [ ] **Step 6: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/crates/shared backend/crates/api
git commit -m "refactor(api): sink AppError into shared crate for cross-crate reuse"
```

---

## Task 2: tasks crate 骨架 + migration

**Files:**
- Modify: `backend/Cargo.toml`（members 加 tasks）
- Create: `backend/crates/tasks/Cargo.toml`
- Create: `backend/crates/tasks/src/lib.rs`
- Create: `backend/crates/tasks/src/models.rs`
- Create: `backend/crates/tasks/src/repo.rs`（占位）
- Create: `backend/crates/db/migrations/0002_tasks.sql`

**Interfaces:**
- Consumes: `shared::AppError`、`sqlx`
- Produces: `tasks` crate 编译通过；migration 0002 建三张表；`tasks::models` 暴露模型与 DTO

- [ ] **Step 1: 注册 workspace member**

修改 `backend/Cargo.toml` 的 `members`：

```toml
members = ["crates/api", "crates/db", "crates/shared", "crates/tasks"]
```

- [ ] **Step 2: 创建 tasks crate 清单**

`backend/crates/tasks/Cargo.toml`：

```toml
[package]
name = "tasks"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
shared = { path = "../shared" }
sqlx.workspace = true
uuid.workspace = true
chrono.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true

[dev-dependencies]
db = { path = "../db" }
tokio = { workspace = true, features = ["test-util"] }
tempfile = "3.27.0"
```

> `db` 只在 dev-dependencies（测试跑 migration）。`tokio` 用 test-util feature 跑 `#[tokio::test]`。

- [ ] **Step 3: 创建 migration 0002**

`backend/crates/db/migrations/0002_tasks.sql`（用 spec 里的 schema）：

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

- [ ] **Step 4: 创建 models.rs**

`backend/crates/tasks/src/models.rs`：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceDetail {
    #[serde(flatten)]
    pub workspace: Workspace,
    pub target_count: i64,
    pub todo_count: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateWorkspace {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateWorkspace {
    pub name: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetStatus {
    Planned,
    Active,
    Done,
    Archived,
}

impl TargetStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetStatus::Planned => "planned",
            TargetStatus::Active => "active",
            TargetStatus::Done => "done",
            TargetStatus::Archived => "archived",
        }
    }
}

impl Default for TargetStatus {
    fn default() -> Self {
        TargetStatus::Planned
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub id: String,
    pub workspace_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTarget {
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateTarget {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TargetStatus>,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Todo,
    Doing,
    Done,
    Blocked,
}

impl TodoStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TodoStatus::Todo => "todo",
            TodoStatus::Doing => "doing",
            TodoStatus::Done => "done",
            TodoStatus::Blocked => "blocked",
        }
    }
}

impl Default for TodoStatus {
    fn default() -> Self {
        TodoStatus::Todo
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Todo {
    pub id: String,
    pub target_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTodo {
    pub title: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateTodo {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TodoStatus>,
    pub sort_order: Option<i64>,
}
```

> 设计说明：
> - `Target.status` / `Todo.status` 序列化为 `String` 出库（DB 存 TEXT，`query_as!` 直接映射 String），前端拿到的是 `"todo"` 等小写串。
> - 入参 `UpdateTarget.status` / `UpdateTodo.status` 用枚举类型，serde 反序列化时非法值（如 `"foo"`）直接报错 -> handler 捕获转 `BadRequest`。这是 Rust 层校验，DB CHECK 是第二道防线。
> - `CreateWorkspace`/`CreateTarget`/`CreateTodo` 字段非 Option，必填。空字符串校验在 repo/handler 层做。

- [ ] **Step 5: 创建 repo.rs 占位 + lib.rs**

`backend/crates/tasks/src/repo.rs`：

```rust
use sqlx::SqlitePool;
use shared::AppResult;

use crate::models::Workspace;

pub async fn list_workspaces(_pool: &SqlitePool) -> AppResult<Vec<Workspace>> {
    unimplemented!()
}
```

`backend/crates/tasks/src/lib.rs`：

```rust
pub mod models;
pub mod repo;

pub use models::*;
```

- [ ] **Step 6: 应用 migration 到 dev.db 并验证**

```powershell
cd backend
$env:VIBE_DB_PATH = "E:\vibe-dashboard\backend\dev.db"
cargo run -p api
```

启动后看到 `server starting` 即 migration 已跑（main.rs 调 `db::run_migrations`）。Ctrl+C 停掉。验证表已建：

```powershell
cd backend
cargo sqlx --help | Out-Null
```

> 若没装 sqlx-cli，L1 Task 3 应已装。用 sqlite 工具确认三张表存在（可选）。

- [ ] **Step 7: 验证编译**

```powershell
cd backend
cargo build -p tasks
cargo clippy --all-targets -- -D warnings
cargo fmt --all
```

Expected: tasks crate 编译通过（repo 是 `unimplemented!()` 占位），clippy 无 warning。

- [ ] **Step 8: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/Cargo.toml backend/crates/tasks backend/crates/db/migrations
git commit -m "feat(tasks): scaffold tasks crate with models and migration 0002"
```

---

## Task 3: Workspace repository 实现

**Files:**
- Modify: `backend/crates/tasks/src/repo.rs`
- Create: `backend/crates/tasks/tests/workspace_test.rs`

**Interfaces:**
- Produces: `repo::{list_workspaces, get_workspace, create_workspace, update_workspace, delete_workspace}`

- [ ] **Step 1: 实现 workspace repo**

替换 `backend/crates/tasks/src/repo.rs` 全文（workspace 部分；target/todo 下个 task 加）：

```rust
use chrono::Utc;
use shared::{AppError, AppResult};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::models::{CreateWorkspace, UpdateWorkspace, Workspace, WorkspaceDetail};

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub async fn list_workspaces(pool: &SqlitePool) -> AppResult<Vec<Workspace>> {
    let rows = sqlx::query_as!(
        Workspace,
        r#"SELECT id, name, path, created_at, updated_at
           FROM workspaces
           ORDER BY updated_at DESC"#
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_workspace(pool: &SqlitePool, id: &str) -> AppResult<WorkspaceDetail> {
    let workspace = sqlx::query_as!(
        Workspace,
        r#"SELECT id, name, path, created_at, updated_at
           FROM workspaces
           WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("workspace {id} not found")))?;

    let (target_count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM targets WHERE workspace_id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    let (todo_count,): (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM todos t
           JOIN targets tg ON t.target_id = tg.id
           WHERE tg.workspace_id = ?"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    Ok(WorkspaceDetail {
        workspace,
        target_count,
        todo_count,
    })
}

pub async fn create_workspace(pool: &SqlitePool, input: CreateWorkspace) -> AppResult<Workspace> {
    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    if input.path.trim().is_empty() {
        return Err(AppError::BadRequest("path must not be empty".into()));
    }

    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    sqlx::query!(
        r#"INSERT INTO workspaces (id, name, path, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?)"#,
        id,
        input.name,
        input.path,
        now,
        now,
    )
    .execute(pool)
    .await?;

    Ok(Workspace {
        id,
        name: input.name,
        path: input.path,
        created_at: now,
        updated_at: now,
    })
}

pub async fn update_workspace(
    pool: &SqlitePool,
    id: &str,
    input: UpdateWorkspace,
) -> AppResult<Workspace> {
    let existing = sqlx::query_as!(
        Workspace,
        r#"SELECT id, name, path, created_at, updated_at
           FROM workspaces WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("workspace {id} not found")))?;

    let name = input.name.unwrap_or(existing.name);
    let path = input.path.unwrap_or(existing.path);

    if name.trim().is_empty() {
        return Err(AppError::BadRequest("name must not be empty".into()));
    }
    if path.trim().is_empty() {
        return Err(AppError::BadRequest("path must not be empty".into()));
    }

    let now = now_rfc3339();
    sqlx::query!(
        r#"UPDATE workspaces SET name = ?, path = ?, updated_at = ? WHERE id = ?"#,
        name,
        path,
        now,
        id,
    )
    .execute(pool)
    .await?;

    Ok(Workspace {
        id: existing.id,
        name,
        path,
        created_at: existing.created_at,
        updated_at: now,
    })
}

pub async fn delete_workspace(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query!("DELETE FROM workspaces WHERE id = ?", id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("workspace {id} not found")));
    }
    Ok(())
}
```

> 说明：
> - `get_workspace` 里的 count 查询用 `query_as`（裸 SQL，无编译期校验，因为是 `COUNT(*)` 元组不映射 struct）。这是可接受的特例，主实体查询都用 `query_as!`。
> - `create_workspace` 返回构造的 Workspace 而非重新查库（INSERT 后数据已知）。`updated_at` 用新时间戳。
> - `delete_workspace` 用 `rows_affected()` 判断是否存在，0 -> NotFound。

- [ ] **Step 2: 写测试辅助 + workspace 测试**

`backend/crates/tasks/tests/workspace_test.rs`：

```rust
use db::{init_pool, run_migrations};
use tasks::repo;
use tasks::CreateWorkspace;

async fn setup_pool() -> sqlx::SqlitePool {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");
    pool
}

// 注意：tempdir 在 setup_pool 返回后即 drop，会把目录删掉。
// 为保持测试期间库文件存在，用以下版本持有 tempdir 句柄。

async fn setup() -> (sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");
    (pool, dir)
}

#[tokio::test]
async fn create_and_get_workspace() {
    let (pool, _dir) = setup().await;
    let ws = repo::create_workspace(&pool, CreateWorkspace {
        name: "demo".into(),
        path: "/tmp/demo".into(),
    })
    .await
    .expect("create");

    let detail = repo::get_workspace(&pool, &ws.id).await.expect("get");
    assert_eq!(detail.workspace.name, "demo");
    assert_eq!(detail.target_count, 0);
    assert_eq!(detail.todo_count, 0);
}

#[tokio::test]
async fn list_workspaces_orders_by_updated_desc() {
    let (pool, _dir) = setup().await;
    let a = repo::create_workspace(&pool, CreateWorkspace { name: "a".into(), path: "/a".into() }).await.unwrap();
    let b = repo::create_workspace(&pool, CreateWorkspace { name: "b".into(), path: "/b".into() }).await.unwrap();
    // 更新 a 让它的 updated_at 更新
    repo::update_workspace(&pool, &a.id, tasks::UpdateWorkspace { name: Some("a2".into()), path: None }).await.unwrap();

    let list = repo::list_workspaces(&pool).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, a.id, "recently updated a should come first");
    assert_eq!(list[1].id, b.id);
}

#[tokio::test]
async fn update_workspace_not_found() {
    let (pool, _dir) = setup().await;
    let err = repo::update_workspace(&pool, "nope", tasks::UpdateWorkspace { name: Some("x".into()), path: None }).await.unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)));
}

#[tokio::test]
async fn create_workspace_rejects_empty_name() {
    let (pool, _dir) = setup().await;
    let err = repo::create_workspace(&pool, CreateWorkspace { name: "  ".into(), path: "/x".into() }).await.unwrap_err();
    assert!(matches!(err, shared::AppError::BadRequest(_)));
}

#[tokio::test]
async fn delete_workspace_not_found() {
    let (pool, _dir) = setup().await;
    let err = repo::delete_workspace(&pool, "nope").await.unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)));
}
```

> 两个 `setup` 函数：第一个 `setup_pool` 未被用可删，保留 `setup` 持有 TempDir。**实现时只留 `setup`。**

- [ ] **Step 3: prepare sqlx 缓存**

```powershell
cd backend
$env:DATABASE_URL = "sqlite:./dev.db"
$env:SQLX_OFFLINE = "false"
cargo sqlx prepare --workspace
$env:SQLX_OFFLINE = "true"
```

Expected: 生成 `backend/.sqlx/` 下 JSON 文件（query-*.json）。commit 这些文件。

> 注意：prepare 前需 `SQLX_OFFLINE=false` 让它连 dev.db 校验。dev.db 必须已跑过 migration（Task 2 Step 6 已做）。prepare 后改回 `true`。

- [ ] **Step 4: 跑测试**

```powershell
cd backend
cargo test -p tasks
```

Expected: 5 个 workspace 测试通过。

- [ ] **Step 5: fmt + clippy**

```powershell
cd backend
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 6: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/crates/tasks backend/.sqlx
git commit -m "feat(tasks): implement workspace repository with query macros"
```

---

## Task 4: Target repository 实现

**Files:**
- Modify: `backend/crates/tasks/src/repo.rs`
- Create: `backend/crates/tasks/tests/target_test.rs`

**Interfaces:**
- Produces: `repo::{list_targets, get_target, create_target, update_target, delete_target}`

- [ ] **Step 1: 追加 target repo 函数**

在 `backend/crates/tasks/src/repo.rs` 末尾追加（保留 workspace 部分）：

```rust
use crate::models::{CreateTarget, Target, TargetStatus, UpdateTarget};

pub async fn list_targets(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<Target>> {
    let rows = sqlx::query_as!(
        Target,
        r#"SELECT id, workspace_id, title, description, status, sort_order, created_at, updated_at
           FROM targets
           WHERE workspace_id = ?
           ORDER BY sort_order ASC, created_at ASC"#,
        workspace_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_target(pool: &SqlitePool, id: &str) -> AppResult<Target> {
    sqlx::query_as!(
        Target,
        r#"SELECT id, workspace_id, title, description, status, sort_order, created_at, updated_at
           FROM targets WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("target {id} not found")))
}

pub async fn create_target(
    pool: &SqlitePool,
    workspace_id: &str,
    input: CreateTarget,
) -> AppResult<Target> {
    if input.title.trim().is_empty() {
        return Err(AppError::BadRequest("title must not be empty".into()));
    }
    // 校验 workspace 存在
    let exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM workspaces WHERE id = ?")
            .bind(workspace_id)
            .fetch_optional(pool)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!(
            "workspace {workspace_id} not found"
        )));
    }

    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let status = TargetStatus::default().as_str();
    sqlx::query!(
        r#"INSERT INTO targets (id, workspace_id, title, description, status, sort_order, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, 0, ?, ?)"#,
        id,
        workspace_id,
        input.title,
        input.description,
        status,
        now,
        now,
    )
    .execute(pool)
    .await?;

    Ok(Target {
        id,
        workspace_id: workspace_id.to_string(),
        title: input.title,
        description: input.description,
        status: status.to_string(),
        sort_order: 0,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn update_target(
    pool: &SqlitePool,
    id: &str,
    input: UpdateTarget,
) -> AppResult<Target> {
    let existing = get_target(pool, id).await?;

    let title = input.title.unwrap_or(existing.title);
    let description = input.description.unwrap_or(existing.description);
    let status = input
        .status
        .map(|s| s.as_str().to_string())
        .unwrap_or(existing.status);
    let sort_order = input.sort_order.unwrap_or(existing.sort_order);

    if title.trim().is_empty() {
        return Err(AppError::BadRequest("title must not be empty".into()));
    }

    let now = now_rfc3339();
    sqlx::query!(
        r#"UPDATE targets SET title = ?, description = ?, status = ?, sort_order = ?, updated_at = ?
           WHERE id = ?"#,
        title,
        description,
        status,
        sort_order,
        now,
        id,
    )
    .execute(pool)
    .await?;

    Ok(Target {
        id: existing.id,
        workspace_id: existing.workspace_id,
        title,
        description,
        status,
        sort_order,
        created_at: existing.created_at,
        updated_at: now,
    })
}

pub async fn delete_target(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query!("DELETE FROM targets WHERE id = ?", id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("target {id} not found")));
    }
    Ok(())
}
```

> `create_target` 用 `query_as` 裸查 workspace 存在性（COUNT/存在性查询不映射 struct，特例）。主实体 CRUD 全用 `query_as!` / `query!`。

- [ ] **Step 2: 写 target 测试**

`backend/crates/tasks/tests/target_test.rs`：

```rust
use db::{init_pool, run_migrations};
use tasks::repo;
use tasks::{CreateTarget, CreateWorkspace, TargetStatus, UpdateTarget};

async fn setup() -> (sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");
    (pool, dir)
}

async fn seed_workspace(pool: &sqlx::SqlitePool) -> String {
    let ws = repo::create_workspace(pool, CreateWorkspace { name: "w".into(), path: "/w".into() })
        .await
        .unwrap();
    ws.id
}

#[tokio::test]
async fn create_and_list_targets() {
    let (pool, _dir) = setup().await;
    let wid = seed_workspace(&pool).await;
    let t = repo::create_target(&pool, &wid, CreateTarget { title: "t1".into(), description: "".into() }).await.unwrap();
    assert_eq!(t.status, "planned");

    let list = repo::list_targets(&pool, &wid).await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, t.id);
}

#[tokio::test]
async fn create_target_workspace_not_found() {
    let (pool, _dir) = setup().await;
    let err = repo::create_target(&pool, "nope", CreateTarget { title: "x".into(), description: "".into() }).await.unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)));
}

#[tokio::test]
async fn update_target_status() {
    let (pool, _dir) = setup().await;
    let wid = seed_workspace(&pool).await;
    let t = repo::create_target(&pool, &wid, CreateTarget { title: "t".into(), description: "".into() }).await.unwrap();

    let updated = repo::update_target(&pool, &t.id, UpdateTarget {
        status: Some(TargetStatus::Done),
        ..Default::default()
    }).await.unwrap();
    assert_eq!(updated.status, "done");
}

#[tokio::test]
async fn delete_target_cascades_todos() {
    let (pool, _dir) = setup().await;
    let wid = seed_workspace(&pool).await;
    let t = repo::create_target(&pool, &wid, CreateTarget { title: "t".into(), description: "".into() }).await.unwrap();
    // 此处依赖 todo repo，下个 task 才有；此测试先简化为只验证 target 删除
    repo::delete_target(&pool, &t.id).await.unwrap();
    let err = repo::get_target(&pool, &t.id).await.unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)));
}
```

> `delete_target_cascades_todos` 的级联验证在 Task 5（todo repo）完成后补全。此处先验证 target 删除本身。实现时可在 Task 5 回来加 todo 断言，或留到此测试时用裸 SQL 插一条 todo 验证级联。**实现者决策：留到 Task 5 补全 todo 断言更自然。**

- [ ] **Step 3: prepare sqlx**

```powershell
cd backend
$env:DATABASE_URL = "sqlite:./dev.db"
$env:SQLX_OFFLINE = "false"
cargo sqlx prepare --workspace
$env:SQLX_OFFLINE = "true"
```

- [ ] **Step 4: 跑测试 + 门禁**

```powershell
cd backend
cargo test -p tasks
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Expected: workspace + target 测试全过。

- [ ] **Step 5: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/crates/tasks backend/.sqlx
git commit -m "feat(tasks): implement target repository"
```

---

## Task 5: Todo repository 实现

**Files:**
- Modify: `backend/crates/tasks/src/repo.rs`
- Modify: `backend/crates/tasks/tests/target_test.rs`（补全级联断言，可选）
- Create: `backend/crates/tasks/tests/todo_test.rs`

**Interfaces:**
- Produces: `repo::{list_todos_by_workspace, list_todos_by_target, get_todo, create_todo, update_todo, delete_todo}`

- [ ] **Step 1: 追加 todo repo 函数**

在 `backend/crates/tasks/src/repo.rs` 末尾追加：

```rust
use crate::models::{CreateTodo, Todo, TodoStatus, UpdateTodo};

pub async fn list_todos_by_workspace(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<Vec<Todo>> {
    let rows = sqlx::query_as!(
        Todo,
        r#"SELECT t.id, t.target_id, t.title, t.description, t.status, t.sort_order, t.created_at, t.updated_at
           FROM todos t
           JOIN targets tg ON t.target_id = tg.id
           WHERE tg.workspace_id = ?
           ORDER BY t.sort_order ASC, t.created_at ASC"#,
        workspace_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn list_todos_by_target(pool: &SqlitePool, target_id: &str) -> AppResult<Vec<Todo>> {
    let rows = sqlx::query_as!(
        Todo,
        r#"SELECT id, target_id, title, description, status, sort_order, created_at, updated_at
           FROM todos
           WHERE target_id = ?
           ORDER BY sort_order ASC, created_at ASC"#,
        target_id
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_todo(pool: &SqlitePool, id: &str) -> AppResult<Todo> {
    sqlx::query_as!(
        Todo,
        r#"SELECT id, target_id, title, description, status, sort_order, created_at, updated_at
           FROM todos WHERE id = ?"#,
        id
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("todo {id} not found")))
}

pub async fn create_todo(
    pool: &SqlitePool,
    target_id: &str,
    input: CreateTodo,
) -> AppResult<Todo> {
    if input.title.trim().is_empty() {
        return Err(AppError::BadRequest("title must not be empty".into()));
    }
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM targets WHERE id = ?")
        .bind(target_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!("target {target_id} not found")));
    }

    let id = Uuid::new_v4().to_string();
    let now = now_rfc3339();
    let status = TodoStatus::default().as_str();
    sqlx::query!(
        r#"INSERT INTO todos (id, target_id, title, description, status, sort_order, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, 0, ?, ?)"#,
        id,
        target_id,
        input.title,
        input.description,
        status,
        now,
        now,
    )
    .execute(pool)
    .await?;

    Ok(Todo {
        id,
        target_id: target_id.to_string(),
        title: input.title,
        description: input.description,
        status: status.to_string(),
        sort_order: 0,
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn update_todo(pool: &SqlitePool, id: &str, input: UpdateTodo) -> AppResult<Todo> {
    let existing = get_todo(pool, id).await?;

    let title = input.title.unwrap_or(existing.title);
    let description = input.description.unwrap_or(existing.description);
    let status = input
        .status
        .map(|s| s.as_str().to_string())
        .unwrap_or(existing.status);
    let sort_order = input.sort_order.unwrap_or(existing.sort_order);

    if title.trim().is_empty() {
        return Err(AppError::BadRequest("title must not be empty".into()));
    }

    let now = now_rfc3339();
    sqlx::query!(
        r#"UPDATE todos SET title = ?, description = ?, status = ?, sort_order = ?, updated_at = ?
           WHERE id = ?"#,
        title,
        description,
        status,
        sort_order,
        now,
        id,
    )
    .execute(pool)
    .await?;

    Ok(Todo {
        id: existing.id,
        target_id: existing.target_id,
        title,
        description,
        status,
        sort_order,
        created_at: existing.created_at,
        updated_at: now,
    })
}

pub async fn delete_todo(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query!("DELETE FROM todos WHERE id = ?", id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("todo {id} not found")));
    }
    Ok(())
}
```

- [ ] **Step 2: 写 todo 测试 + 级联测试**

`backend/crates/tasks/tests/todo_test.rs`：

```rust
use db::{init_pool, run_migrations};
use tasks::repo;
use tasks::{CreateTarget, CreateTodo, CreateWorkspace, TodoStatus, UpdateTodo};

async fn setup() -> (sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir failed");
    let db_path = dir.path().join("test.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");
    (pool, dir)
}

async fn seed(pool: &sqlx::SqlitePool) -> (String, String) {
    let ws = repo::create_workspace(pool, CreateWorkspace { name: "w".into(), path: "/w".into() }).await.unwrap();
    let t = repo::create_target(pool, &ws.id, CreateTarget { title: "t".into(), description: "".into() }).await.unwrap();
    (ws.id, t.id)
}

#[tokio::test]
async fn create_and_get_todo() {
    let (pool, _dir) = setup().await;
    let (_wid, tid) = seed(&pool).await;
    let todo = repo::create_todo(&pool, &tid, CreateTodo { title: "do thing".into(), description: "desc".into() }).await.unwrap();
    assert_eq!(todo.status, "todo");

    let got = repo::get_todo(&pool, &todo.id).await.unwrap();
    assert_eq!(got.title, "do thing");
}

#[tokio::test]
async fn list_todos_by_workspace_cross_target() {
    let (pool, _dir) = setup().await;
    let (wid, tid) = seed(&pool).await;
    // 第二个 target
    let t2 = repo::create_target(&pool, &wid, CreateTarget { title: "t2".into(), description: "".into() }).await.unwrap();
    repo::create_todo(&pool, &tid, CreateTodo { title: "a".into(), description: "".into() }).await.unwrap();
    repo::create_todo(&pool, &t2.id, CreateTodo { title: "b".into(), description: "".into() }).await.unwrap();

    let todos = repo::list_todos_by_workspace(&pool, &wid).await.unwrap();
    assert_eq!(todos.len(), 2);
}

#[tokio::test]
async fn update_todo_status() {
    let (pool, _dir) = setup().await;
    let (_wid, tid) = seed(&pool).await;
    let todo = repo::create_todo(&pool, &tid, CreateTodo { title: "x".into(), description: "".into() }).await.unwrap();

    let updated = repo::update_todo(&pool, &todo.id, UpdateTodo {
        status: Some(TodoStatus::Doing),
        ..Default::default()
    }).await.unwrap();
    assert_eq!(updated.status, "doing");
}

#[tokio::test]
async fn delete_target_cascades_todos() {
    let (pool, _dir) = setup().await;
    let (_wid, tid) = seed(&pool).await;
    let todo = repo::create_todo(&pool, &tid, CreateTodo { title: "x".into(), description: "".into() }).await.unwrap();

    repo::delete_target(&pool, &tid).await.unwrap();
    let err = repo::get_todo(&pool, &todo.id).await.unwrap_err();
    assert!(matches!(err, shared::AppError::NotFound(_)), "todo should be gone after target cascade delete");
}

#[tokio::test]
async fn delete_workspace_cascades_targets_and_todos() {
    let (pool, _dir) = setup().await;
    let (wid, tid) = seed(&pool).await;
    let todo = repo::create_todo(&pool, &tid, CreateTodo { title: "x".into(), description: "".into() }).await.unwrap();

    repo::delete_workspace(&pool, &wid).await.unwrap();
    assert!(matches!(repo::get_target(&pool, &tid).await.unwrap_err(), shared::AppError::NotFound(_)));
    assert!(matches!(repo::get_todo(&pool, &todo.id).await.unwrap_err(), shared::AppError::NotFound(_)));
}
```

- [ ] **Step 3: prepare sqlx**

```powershell
cd backend
$env:DATABASE_URL = "sqlite:./dev.db"
$env:SQLX_OFFLINE = "false"
cargo sqlx prepare --workspace
$env:SQLX_OFFLINE = "true"
```

- [ ] **Step 4: 跑测试 + 门禁**

```powershell
cd backend
cargo test -p tasks
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Expected: workspace + target + todo 全部测试通过（含级联删除）。

- [ ] **Step 5: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/crates/tasks backend/.sqlx
git commit -m "feat(tasks): implement todo repository with cascade delete tests"
```

---

## Task 6: API 路由 - Workspace handlers

**Files:**
- Modify: `backend/crates/api/Cargo.toml`（加 tasks 依赖）
- Create: `backend/crates/api/src/routes/tasks.rs`
- Modify: `backend/crates/api/src/routes/mod.rs`
- Modify: `backend/crates/api/src/main.rs`（build_router 加路由）
- Modify: `backend/crates/api/src/state.rs`（去掉 db 的 dead_code allow）

**Interfaces:**
- Produces: `GET/POST/GET/PUT/DELETE /api/workspaces` 端点可用

- [ ] **Step 1: api 依赖 tasks**

修改 `backend/crates/api/Cargo.toml` 的 `[dependencies]`，加入：

```toml
tasks = { path = "../tasks" }
```

放在 `db = { path = "../db" }` 后面。

- [ ] **Step 2: 创建 tasks 路由文件（workspace 部分）**

`backend/crates/api/src/routes/tasks.rs`：

```rust
use axum::{
    extract::{Path, State},
    Json,
};
use shared::AppResult;

use crate::state::AppState;
use tasks::{CreateWorkspace, UpdateWorkspace, WorkspaceDetail};

pub async fn list_workspaces(State(state): State<AppState>) -> AppResult<Json<Vec<tasks::Workspace>>> {
    let rows = tasks::repo::list_workspaces(&state.db).await?;
    Ok(Json(rows))
}

pub async fn create_workspace(
    State(state): State<AppState>,
    Json(input): Json<CreateWorkspace>,
) -> AppResult<Json<tasks::Workspace>> {
    let ws = tasks::repo::create_workspace(&state.db, input).await?;
    Ok(Json(ws))
}

pub async fn get_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<WorkspaceDetail>> {
    let detail = tasks::repo::get_workspace(&state.db, &id).await?;
    Ok(Json(detail))
}

pub async fn update_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateWorkspace>,
) -> AppResult<Json<tasks::Workspace>> {
    let ws = tasks::repo::update_workspace(&state.db, &id, input).await?;
    Ok(Json(ws))
}

pub async fn delete_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<()> {
    tasks::repo::delete_workspace(&state.db, &id).await?;
    Ok(())
}
```

> `delete_workspace` 返回 `AppResult<()>`。axum 对 `()` 的响应是 200 空体。spec 要求 204 -- 在 router 层用 `.map(status_code)` 或返回 `(StatusCode::NO_CONTENT, ())`。**实现时改为返回 `AppResult<StatusCode>`：**
>
> 修正：`delete_workspace` 返回 `AppResult<axum::http::StatusCode>`，函数体 `Ok(axum::http::StatusCode::NO_CONTENT)`。这样 axum 渲染 204。
>
> **所有 delete handler 统一用此模式。** 下方 Step 2 最终版本：

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use shared::AppResult;

use crate::state::AppState;
use tasks::{CreateWorkspace, UpdateWorkspace, WorkspaceDetail};

pub async fn list_workspaces(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<tasks::Workspace>>> {
    let rows = tasks::repo::list_workspaces(&state.db).await?;
    Ok(Json(rows))
}

pub async fn create_workspace(
    State(state): State<AppState>,
    Json(input): Json<CreateWorkspace>,
) -> AppResult<Json<tasks::Workspace>> {
    let ws = tasks::repo::create_workspace(&state.db, input).await?;
    Ok(Json(ws))
}

pub async fn get_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<WorkspaceDetail>> {
    let detail = tasks::repo::get_workspace(&state.db, &id).await?;
    Ok(Json(detail))
}

pub async fn update_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateWorkspace>,
) -> AppResult<Json<tasks::Workspace>> {
    let ws = tasks::repo::update_workspace(&state.db, &id, input).await?;
    Ok(Json(ws))
}

pub async fn delete_workspace(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    tasks::repo::delete_workspace(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 3: 注册路由模块 + router**

修改 `backend/crates/api/src/routes/mod.rs`：

```rust
pub mod health;
pub mod tasks;
pub mod ws;
```

修改 `backend/crates/api/src/main.rs` 的 `build_router`：

```rust
fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(routes::health::health))
        .route(
            "/api/workspaces",
            get(routes::tasks::list_workspaces).post(routes::tasks::create_workspace),
        )
        .route(
            "/api/workspaces/:id",
            get(routes::tasks::get_workspace)
                .put(routes::tasks::update_workspace)
                .delete(routes::tasks::delete_workspace),
        )
        .route("/ws", get(routes::ws::ws_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
```

> axum 0.7 路径参数用 `:id` 语法（非 `{id}`，那是 0.8）。

- [ ] **Step 4: 去掉 state.rs 的 dead_code allow**

修改 `backend/crates/api/src/state.rs`，删除 `pub db` 字段上的 `#[allow(dead_code)]`：

```rust
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub hub: Arc<Hub>,
    #[allow(dead_code)]
    pub config: Arc<Config>,
    pub started_at: DateTime<Utc>,
}
```

> `config` 仍未被路由用，保留其 allow。`db` 现在 tasks 路由会用，去掉 allow。

- [ ] **Step 5: 编译 + 手动验证**

```powershell
cd backend
cargo build -p api
```

```powershell
cd backend
$env:VIBE_DB_PATH = "E:\vibe-dashboard\backend\dev.db"
cargo run -p api
```

另开终端：

```powershell
curl http://127.0.0.1:8787/api/workspaces
# 预期: []

# 创建
curl -Method POST http://127.0.0.1:8787/api/workspaces -ContentType "application/json" -Body '{"name":"demo","path":"E:/demo"}'
# 预期: {"id":"...","name":"demo","path":"E:/demo","created_at":"...","updated_at":"..."}

# 列表
curl http://127.0.0.1:8787/api/workspaces
# 预期: 含刚建的 workspace

# 删除
curl -Method DELETE http://127.0.0.1:8787/api/workspaces/<id>
# 预期: 204
```

> PowerShell 的 `curl` 是 `Invoke-WebRequest` 别名。用 `Invoke-RestMethod` 更顺：
> `Invoke-RestMethod -Uri http://127.0.0.1:8787/api/workspaces -Method POST -ContentType "application/json" -Body '{"name":"demo","path":"E:/demo"}'`

- [ ] **Step 6: fmt + clippy**

```powershell
cd backend
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

- [ ] **Step 7: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/crates/api
git commit -m "feat(api): add workspace CRUD routes"
```

---

## Task 7: API 路由 - Target & Todo handlers

**Files:**
- Modify: `backend/crates/api/src/routes/tasks.rs`
- Modify: `backend/crates/api/src/main.rs`（build_router 加 target/todo 路由）

**Interfaces:**
- Produces: target 和 todo 的全部 CRUD 端点

- [ ] **Step 1: 追加 target + todo handlers**

在 `backend/crates/api/src/routes/tasks.rs` 末尾追加：

```rust
use tasks::{CreateTarget, CreateTodo, UpdateTarget, UpdateTodo};

// ---------- Targets ----------

pub async fn list_targets(
    State(state): State<AppState>,
    Path(wid): Path<String>,
) -> AppResult<Json<Vec<tasks::Target>>> {
    Ok(Json(tasks::repo::list_targets(&state.db, &wid).await?))
}

pub async fn create_target(
    State(state): State<AppState>,
    Path(wid): Path<String>,
    Json(input): Json<CreateTarget>,
) -> AppResult<Json<tasks::Target>> {
    Ok(Json(tasks::repo::create_target(&state.db, &wid, input).await?))
}

pub async fn get_target(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<tasks::Target>> {
    Ok(Json(tasks::repo::get_target(&state.db, &id).await?))
}

pub async fn update_target(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateTarget>,
) -> AppResult<Json<tasks::Target>> {
    Ok(Json(tasks::repo::update_target(&state.db, &id, input).await?))
}

pub async fn delete_target(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    tasks::repo::delete_target(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------- Todos ----------

pub async fn list_todos_by_workspace(
    State(state): State<AppState>,
    Path(wid): Path<String>,
) -> AppResult<Json<Vec<tasks::Todo>>> {
    Ok(Json(tasks::repo::list_todos_by_workspace(&state.db, &wid).await?))
}

pub async fn list_todos_by_target(
    State(state): State<AppState>,
    Path(tid): Path<String>,
) -> AppResult<Json<Vec<tasks::Todo>>> {
    Ok(Json(tasks::repo::list_todos_by_target(&state.db, &tid).await?))
}

pub async fn create_todo(
    State(state): State<AppState>,
    Path(tid): Path<String>,
    Json(input): Json<CreateTodo>,
) -> AppResult<Json<tasks::Todo>> {
    Ok(Json(tasks::repo::create_todo(&state.db, &tid, input).await?))
}

pub async fn get_todo(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<tasks::Todo>> {
    Ok(Json(tasks::repo::get_todo(&state.db, &id).await?))
}

pub async fn update_todo(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateTodo>,
) -> AppResult<Json<tasks::Todo>> {
    Ok(Json(tasks::repo::update_todo(&state.db, &id, input).await?))
}

pub async fn delete_todo(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    tasks::repo::delete_todo(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 2: 注册路由**

修改 `backend/crates/api/src/main.rs` 的 `build_router`，在 `/ws` 路由前插入：

```rust
fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(routes::health::health))
        // workspaces
        .route(
            "/api/workspaces",
            get(routes::tasks::list_workspaces).post(routes::tasks::create_workspace),
        )
        .route(
            "/api/workspaces/:id",
            get(routes::tasks::get_workspace)
                .put(routes::tasks::update_workspace)
                .delete(routes::tasks::delete_workspace),
        )
        // targets
        .route(
            "/api/workspaces/:wid/targets",
            get(routes::tasks::list_targets).post(routes::tasks::create_target),
        )
        .route(
            "/api/targets/:id",
            get(routes::tasks::get_target)
                .put(routes::tasks::update_target)
                .delete(routes::tasks::delete_target),
        )
        // todos
        .route(
            "/api/workspaces/:wid/todos",
            get(routes::tasks::list_todos_by_workspace),
        )
        .route(
            "/api/targets/:tid/todos",
            get(routes::tasks::list_todos_by_target).post(routes::tasks::create_todo),
        )
        .route(
            "/api/todos/:id",
            get(routes::tasks::get_todo)
                .put(routes::tasks::update_todo)
                .delete(routes::tasks::delete_todo),
        )
        .route("/ws", get(routes::ws::ws_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
```

- [ ] **Step 3: 编译 + 手动联调**

```powershell
cd backend
cargo build -p api
$env:VIBE_DB_PATH = "E:\vibe-dashboard\backend\dev.db"
cargo run -p api
```

另开终端（PowerShell `Invoke-RestMethod`）：

```powershell
$ws = Invoke-RestMethod -Uri http://127.0.0.1:8787/api/workspaces -Method POST -ContentType "application/json" -Body '{"name":"demo","path":"E:/demo"}'
$t = Invoke-RestMethod -Uri "http://127.0.0.1:8787/api/workspaces/$($ws.id)/targets" -Method POST -ContentType "application/json" -Body '{"title":"t1"}'
$todo = Invoke-RestMethod -Uri "http://127.0.0.1:8787/api/targets/$($t.id)/todos" -Method POST -ContentType "application/json" -Body '{"title":"do x"}'
Invoke-RestMethod -Uri "http://127.0.0.1:8787/api/workspaces/$($ws.id)/todos"
# 预期: 含 todo

# 非法 status -> 400
try { Invoke-RestMethod -Uri "http://127.0.0.1:8787/api/todos/$($todo.id)" -Method PUT -ContentType "application/json" -Body '{"status":"bogus"}' } catch { $_.Exception.Response.StatusCode.value__ }
# 预期: 400

# 不存在 -> 404
try { Invoke-RestMethod -Uri "http://127.0.0.1:8787/api/todos/nope" } catch { $_.Exception.Response.StatusCode.value__ }
# 预期: 404
```

- [ ] **Step 4: 写 api handler 测试**

`backend/crates/api/tests/tasks_api_test.rs`：

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use db::{init_pool, run_migrations};
use tower::ServiceExt;

use api::app; // 需暴露 app builder，见 Step 5

async fn setup_app() -> (axum::Router, sqlx::SqlitePool, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("t.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let hub = api::Hub::new(); // 需暴露，见 Step 5
    let state = api::AppState::new(pool.clone(), hub, api::Config::default_for_test());
    let app = api::app(state);
    (app, pool, dir)
}
```

> 此测试需要 api crate 暴露 `app(state) -> Router`、`Hub`、`AppState`、`Config`。**这是重构点**，见 Step 5。

- [ ] **Step 5: 暴露 app builder 供测试**

为了让 handler 测试能构造 router，重构 `main.rs`：把 `build_router` 改为 `pub fn app(state: AppState) -> Router` 并在 `lib.rs` 暴露。但 api 是 binary crate（有 main.rs）。**两种方案：**

**方案 A（推荐）**：api crate 同时有 `lib.rs` + `main.rs`。`lib.rs` 暴露 `pub fn app(state)`、re-export `AppState`/`Hub`/`Config`。`main.rs` 调 `api::app`。测试用 `api::app`。

修改 `backend/crates/api/Cargo.toml` 加：

```toml
[lib]
name = "api"
path = "src/lib.rs"

[[bin]]
name = "api"
path = "src/main.rs"
```

创建 `backend/crates/api/src/lib.rs`：

```rust
pub mod config;
pub mod error;
pub mod routes;
pub mod state;
pub mod ws;

pub use config::Config;
pub use state::AppState;
pub use ws::Hub;

use axum::{routing::get, Router};
use tower_http::trace::TraceLayer;

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(routes::health::health))
        // ... 全部路由（从 main.rs build_router 搬来）
        .route("/ws", get(routes::ws::ws_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
```

修改 `main.rs`：删 `mod` 声明和 `build_router`，改为调 `api::app`：

```rust
use api::Config;
use shared::logging;
// ...

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    logging::init(&config.log_level);
    // ... pool, migrations, hub, state ...
    let app = api::app(state.clone());
    // ... serve ...
}
```

> `main.rs` 现在通过 crate 名 `api` 引用 lib。注意 binary 和 lib 同名 `api`，`main.rs` 里用 `api::` 路径访问 lib 的公开项。`mod config;` 等声明移到 `lib.rs`，`main.rs` 不再声明 mod。

**方案 B**：不重构，handler 测试用 `tower::ServiceExt::oneshot` 直接测 repo 层（已在 tasks crate 测过）。**但 spec 要求 api handler 测试。** 选方案 A。

`Config` 需要一个测试构造器。在 `config.rs` 加：

```rust
#[cfg(test)]
impl Config {
    pub fn default_for_test() -> Self {
        Self {
            db_path: String::new(),
            http_port: 0,
            log_level: "info".into(),
        }
    }
}
```

- [ ] **Step 6: 完善 handler 测试**

`backend/crates/api/tests/tasks_api_test.rs`（完整版）：

```rust
use axum::body::Body;
use axum::http::{Request, StatusCode};
use db::{init_pool, run_migrations};
use tower::ServiceExt;

async fn setup() -> (axum::Router, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("t.db");
    let url = format!("sqlite://{}", db_path.to_string_lossy().replace('\\', "/"));
    let pool = init_pool(&url).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let hub = api::Hub::new();
    let state = api::AppState::new(pool, hub, api::Config::default_for_test());
    (api::app(state), dir)
}

#[tokio::test]
async fn workspace_crud_roundtrip() {
    let (app, _dir) = setup().await;

    // create
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workspaces")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"demo","path":"/d"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let ws: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = ws["id"].as_str().unwrap().to_string();

    // list
    let resp = app
        .clone()
        .oneshot(Request::builder().uri("/api/workspaces").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // delete
    let resp = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/workspaces/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn invalid_status_returns_400() {
    let (app, _dir) = setup().await;
    // 先建 workspace + target + todo（省略，用 repo 直接 seed 或串请求）
    // 简化：直接 PUT 非法 status 到不存在的 todo 也会先 404。
    // 此测试改为验证非法 JSON body 反序列化 -> 400。
    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workspaces")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"  "}"#)) // 缺 path
                .unwrap(),
        )
        .await
        .unwrap();
    // 缺字段 -> serde 反序列化失败 -> axum 默认 400
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
```

> api crate 需在 dev-dependencies 加 `tempfile`、`tower`（已有）、`serde_json`（已有）。`tempfile` 加到 `[dev-dependencies]`。
>
> 实现者注意：`invalid_status_returns_400` 测试非法 status 需要先 seed 一条 todo 再 PUT。上面的简化版测的是缺字段 400。**实现时补一个真正测非法 status 的：seed todo 后 PUT `{"status":"bogus"}` 期望 400。**

- [ ] **Step 7: prepare sqlx（api crate 也用 query 间接触发？不直接用，但保险）**

api crate 不直接用 `query!`，无需 prepare。但 `cargo sqlx prepare --workspace` 会扫所有 crate，确认无遗漏：

```powershell
cd backend
$env:DATABASE_URL = "sqlite:./dev.db"
$env:SQLX_OFFLINE = "false"
cargo sqlx prepare --workspace
$env:SQLX_OFFLINE = "true"
```

- [ ] **Step 8: 跑测试 + 门禁**

```powershell
cd backend
cargo test
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Expected: tasks + api + db + shared 全部测试通过。

- [ ] **Step 9: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/crates/api backend/.sqlx
git commit -m "feat(api): add target/todo CRUD routes and handler tests"
```

---

## Task 8: 前端类型 + API hooks + shadcn 组件

**Files:**
- Modify: `frontend/src/types/api.ts`
- Create: `frontend/src/hooks/useWorkspaces.ts`
- Create: `frontend/src/hooks/useTargets.ts`
- Create: `frontend/src/hooks/useTodos.ts`
- Modify: `frontend/src/lib/api.ts`（del 处理 204）
- 新增 shadcn 组件：dialog, input, textarea, label, select, dropdown-menu

**Interfaces:**
- Produces: 前端类型定义、TanStack Query hooks、`del` 支持 204 空响应

- [ ] **Step 1: 安装 react-router-dom + shadcn 组件**

```powershell
cd frontend
npm install react-router-dom@6
npx shadcn@latest add dialog input textarea label select dropdown-menu
```

> React 19 兼容性：shadcn 组件依赖 radix-ui，多数已支持 React 19。若 add 时报 peer dep 警告，用 `--legacy-peer-deps` 或按提示处理。

- [ ] **Step 2: 扩展类型定义**

替换 `frontend/src/types/api.ts`（保留 L1 的 health/ws 类型，追加业务类型）：

```ts
export interface HealthResponse {
  status: string;
  version: string;
  uptime_seconds: number;
}

export interface HelloPayload {
  connection_id: string;
  server_time: string;
}

export interface PongPayload {
  server_time: string;
}

export type ServerMsg =
  | { type: "hello"; payload: HelloPayload }
  | { type: "pong"; payload: PongPayload };

export type ClientMsg = { type: "ping" };

// ---------- L2 业务类型 ----------

export interface Workspace {
  id: string;
  name: string;
  path: string;
  created_at: string;
  updated_at: string;
}

export interface WorkspaceDetail extends Workspace {
  target_count: number;
  todo_count: number;
}

export interface CreateWorkspace {
  name: string;
  path: string;
}

export interface UpdateWorkspace {
  name?: string;
  path?: string;
}

export type TargetStatus = "planned" | "active" | "done" | "archived";

export interface Target {
  id: string;
  workspace_id: string;
  title: string;
  description: string;
  status: TargetStatus;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

export interface CreateTarget {
  title: string;
  description?: string;
}

export interface UpdateTarget {
  title?: string;
  description?: string;
  status?: TargetStatus;
  sort_order?: number;
}

export type TodoStatus = "todo" | "doing" | "done" | "blocked";

export interface Todo {
  id: string;
  target_id: string;
  title: string;
  description: string;
  status: TodoStatus;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

export interface CreateTodo {
  title: string;
  description?: string;
}

export interface UpdateTodo {
  title?: string;
  description?: string;
  status?: TodoStatus;
  sort_order?: number;
}
```

- [ ] **Step 3: 修复 del 支持 204**

修改 `frontend/src/lib/api.ts` 的 `request` 函数，处理 204 No Content（无 body）：

```ts
async function request<T>(
  method: string,
  path: string,
  body?: unknown,
): Promise<T> {
  const res = await fetch(path, {
    method,
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });

  if (!res.ok) {
    let message = res.statusText;
    try {
      const data = await res.json();
      message = data.error ?? message;
    } catch {
      // body not json
    }
    throw new ApiError(res.status, message);
  }

  if (res.status === 204) {
    return undefined as T;
  }
  return res.json() as Promise<T>;
}
```

> DELETE 返回 204 无 body，原 `res.json()` 会抛 JSON 解析错。加 204 短路。`del<T>` 仍返回 `Promise<T>`，调用方传 `del<void>` 或忽略返回。

- [ ] **Step 4: 写 hooks**

`frontend/src/hooks/useWorkspaces.ts`：

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { del, getJson, postJson, putJson } from "@/lib/api";
import type { CreateWorkspace, UpdateWorkspace, Workspace } from "@/types/api";

export function useWorkspaces() {
  return useQuery({
    queryKey: ["workspaces"],
    queryFn: () => getJson<Workspace[]>("/api/workspaces"),
  });
}

export function useCreateWorkspace() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateWorkspace) =>
      postJson<Workspace>("/api/workspaces", input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["workspaces"] }),
  });
}

export function useUpdateWorkspace(id: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: UpdateWorkspace) =>
      putJson<Workspace>(`/api/workspaces/${id}`, input),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["workspaces"] });
      qc.invalidateQueries({ queryKey: ["workspace", id] });
    },
  });
}

export function useDeleteWorkspace() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => del<void>(`/api/workspaces/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["workspaces"] }),
  });
}
```

`frontend/src/hooks/useTargets.ts`：

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { del, getJson, postJson, putJson } from "@/lib/api";
import type { CreateTarget, Target, UpdateTarget } from "@/types/api";

export function useTargets(workspaceId: string) {
  return useQuery({
    queryKey: ["targets", workspaceId],
    queryFn: () => getJson<Target[]>(`/api/workspaces/${workspaceId}/targets`),
    enabled: !!workspaceId,
  });
}

export function useCreateTarget(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateTarget) =>
      postJson<Target>(`/api/workspaces/${workspaceId}/targets`, input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["targets", workspaceId] }),
  });
}

export function useUpdateTarget(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdateTarget }) =>
      putJson<Target>(`/api/targets/${id}`, input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["targets", workspaceId] }),
  });
}

export function useDeleteTarget(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => del<void>(`/api/targets/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["targets", workspaceId] }),
  });
}
```

`frontend/src/hooks/useTodos.ts`：

```ts
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { del, getJson, postJson, putJson } from "@/lib/api";
import type { CreateTodo, Todo, UpdateTodo } from "@/types/api";

export function useTodos(workspaceId: string) {
  return useQuery({
    queryKey: ["todos", workspaceId],
    queryFn: () => getJson<Todo[]>(`/api/workspaces/${workspaceId}/todos`),
    enabled: !!workspaceId,
  });
}

export function useCreateTodo(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ targetId, input }: { targetId: string; input: CreateTodo }) =>
      postJson<Todo>(`/api/targets/${targetId}/todos`, input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["todos", workspaceId] }),
  });
}

export function useUpdateTodo(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdateTodo }) =>
      putJson<Todo>(`/api/todos/${id}`, input),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["todos", workspaceId] }),
  });
}

export function useDeleteTodo(workspaceId: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => del<void>(`/api/todos/${id}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["todos", workspaceId] }),
  });
}
```

- [ ] **Step 5: typecheck**

```powershell
cd frontend
npm run typecheck
```

Expected: 无错误（hooks 暂未被组件用，但应通过类型检查）。

- [ ] **Step 6: Commit**

```powershell
cd E:\vibe-dashboard
git add frontend
git commit -m "feat(frontend): add task types, query hooks, and shadcn form components"
```

---

## Task 9: 全局状态 hook + Sidebar 改造 + App 路由

**Files:**
- Create: `frontend/src/hooks/useGlobalStatus.ts`
- Modify: `frontend/src/App.tsx`（Router + 全局 hook）
- Modify: `frontend/src/components/layout/Sidebar.tsx`（workspace 导航 + 状态圆点）
- Delete: `frontend/src/pages/HomePage.tsx`
- Create: `frontend/src/pages/WorkspacesPage.tsx`（占位，Task 10 填实）
- Create: `frontend/src/pages/WorkspaceViewPage.tsx`（占位，Task 11 填实）

**Interfaces:**
- Produces: 路由骨架，Sidebar 显示 workspace 列表 + 底部 health/ws 圆点，L1 HomePage 删除

- [ ] **Step 1: 全局状态 hook**

`frontend/src/hooks/useGlobalStatus.ts`（把 L1 HomePage 的 health query + ws 订阅迁过来，挂 App 顶层）：

```ts
import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { getJson } from "@/lib/api";
import { wsClient } from "@/lib/ws";
import { useUiStore } from "@/stores/ui";
import type { HealthResponse, ServerMsg } from "@/types/api";

export function useGlobalStatus() {
  const { setWsStatus, setConnectionId, setPingPongLatency } = useUiStore();
  const pingSentAtRef = useRef<number | null>(null);

  const healthQuery = useQuery({
    queryKey: ["health"],
    queryFn: () => getJson<HealthResponse>("/api/health"),
    refetchInterval: 5000,
  });

  useEffect(() => {
    wsClient.connect(
      `${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`,
    );

    const unsubStatus = wsClient.onStatus((status) => {
      setWsStatus(status);
      if (status === "open") {
        pingSentAtRef.current = null;
        setPingPongLatency(0);
      }
    });

    const unsubMsg = wsClient.subscribe((msg: ServerMsg) => {
      if (msg.type === "hello") {
        setConnectionId(msg.payload.connection_id);
      } else if (msg.type === "pong") {
        if (pingSentAtRef.current != null) {
          setPingPongLatency(Date.now() - pingSentAtRef.current);
          pingSentAtRef.current = null;
        }
      }
    });

    return () => {
      unsubStatus();
      unsubMsg();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return {
    healthOk: healthQuery.data?.status === "ok",
  };
}
```

> `healthOk` 供 Sidebar 圆点用。ws 状态从 `useUiStore.wsStatus` 读（Sidebar 直接订阅 store）。
> ping 按钮的 `handlePing` 留在 HomePage 原地删除后不保留（L2 无 ping 按钮 UI）。若想保留 ping 测试，可在 Sidebar 加，但 spec 未要求。**不保留 ping 按钮。**

- [ ] **Step 2: 改造 Sidebar**

替换 `frontend/src/components/layout/Sidebar.tsx`：

```tsx
import { NavLink } from "react-router-dom";
import { useWorkspaces } from "@/hooks/useWorkspaces";
import { useUiStore } from "@/stores/ui";

function StatusDot({ ok, label }: { ok: boolean; label: string }) {
  return (
    <div className="flex items-center gap-2">
      <span
        className={`h-2.5 w-2.5 rounded-full ${ok ? "bg-green-500" : "bg-red-500"}`}
      />
      <span className="text-xs text-muted-foreground">{label}</span>
    </div>
  );
}

export function Sidebar({ healthOk }: { healthOk: boolean }) {
  const { data: workspaces } = useWorkspaces();
  const wsStatus = useUiStore((s) => s.wsStatus);

  return (
    <aside className="flex w-60 flex-col border-r bg-card min-h-screen p-4">
      <h2 className="text-lg font-semibold mb-4">Vibe Dashboard</h2>
      <nav className="space-y-1 flex-1">
        <NavLink
          to="/"
          end
          className={({ isActive }) =>
            `block rounded-md px-2 py-1.5 text-sm hover:bg-accent ${
              isActive ? "bg-accent font-medium" : ""
            }`
          }
        >
          所有工作区
        </NavLink>
        {workspaces?.map((ws) => (
          <NavLink
            key={ws.id}
            to={`/workspaces/${ws.id}`}
            className={({ isActive }) =>
              `block rounded-md px-2 py-1.5 text-sm hover:bg-accent truncate ${
                isActive ? "bg-accent font-medium" : ""
              }`
            }
          >
            {ws.name}
          </NavLink>
        ))}
      </nav>
      <div className="space-y-2 border-t pt-3">
        <StatusDot ok={healthOk} label="后端" />
        <StatusDot ok={wsStatus === "open"} label="WebSocket" />
      </div>
    </aside>
  );
}
```

> `healthOk` 由 App 传入（来自 `useGlobalStatus`）。ws 状态直接从 Zustand store 读。Sidebar 用 flex-col，导航区 `flex-1` 撑开，圆点固定底部。

- [ ] **Step 3: 改造 App.tsx（路由 + 全局 hook）**

替换 `frontend/src/App.tsx`：

```tsx
import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient } from "@/lib/query";
import { Sidebar } from "@/components/layout/Sidebar";
import { useGlobalStatus } from "@/hooks/useGlobalStatus";
import { WorkspacesPage } from "@/pages/WorkspacesPage";
import { WorkspaceViewPage } from "@/pages/WorkspaceViewPage";

function App() {
  const { healthOk } = useGlobalStatus();

  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <div className="flex min-h-screen">
          <Sidebar healthOk={healthOk} />
          <main className="flex-1">
            <Routes>
              <Route path="/" element={<WorkspacesPage />} />
              <Route path="/workspaces/:wid" element={<WorkspaceViewPage />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </main>
        </div>
      </BrowserRouter>
    </QueryClientProvider>
  );
}

export default App;
```

> `useGlobalStatus` 在 Router 外层调用（它不依赖路由，挂 App 顶层连接生命周期最稳）。

- [ ] **Step 4: 占位页面 + 删除 HomePage**

删除 `frontend/src/pages/HomePage.tsx`。

创建占位 `frontend/src/pages/WorkspacesPage.tsx`：

```tsx
export function WorkspacesPage() {
  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-6">工作区</h1>
      <p className="text-muted-foreground">Task 10 填实</p>
    </div>
  );
}
```

创建占位 `frontend/src/pages/WorkspaceViewPage.tsx`：

```tsx
import { useParams } from "react-router-dom";

export function WorkspaceViewPage() {
  const { wid } = useParams();
  return (
    <div className="p-6">
      <h1 className="text-2xl font-bold mb-6">看板 {wid}</h1>
      <p className="text-muted-foreground">Task 11 填实</p>
    </div>
  );
}
```

- [ ] **Step 5: typecheck + build**

```powershell
cd frontend
npm run typecheck
npm run build
```

Expected: 通过。删除 HomePage 后无悬空 import。

- [ ] **Step 6: Commit**

```powershell
cd E:\vibe-dashboard
git add frontend
git commit -m "feat(frontend): add router, global status hook, and sidebar with status dots"
```

---

## Task 10: WorkspacesPage 实现

**Files:**
- Modify: `frontend/src/pages/WorkspacesPage.tsx`
- Create: `frontend/src/components/workspace/WorkspaceCard.tsx`
- Create: `frontend/src/components/workspace/CreateWorkspaceDialog.tsx`

**Interfaces:**
- Produces: `/` 显示 workspace 卡片网格 + 新建 dialog，点击进入看板

- [ ] **Step 1: CreateWorkspaceDialog**

`frontend/src/components/workspace/CreateWorkspaceDialog.tsx`：

```tsx
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useCreateWorkspace } from "@/hooks/useWorkspaces";

export function CreateWorkspaceDialog() {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [path, setPath] = useState("");
  const navigate = useNavigate();
  const create = useCreateWorkspace();

  const handleSubmit = async () => {
    if (!name.trim() || !path.trim()) return;
    const ws = await create.mutateAsync({ name: name.trim(), path: path.trim() });
    setName("");
    setPath("");
    setOpen(false);
    navigate(`/workspaces/${ws.id}`);
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>新建工作区</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>新建工作区</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 py-2">
          <div className="space-y-2">
            <Label htmlFor="ws-name">名称</Label>
            <Input
              id="ws-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="我的项目"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="ws-path">路径</Label>
            <Input
              id="ws-path"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="E:/projects/my-project"
            />
          </div>
        </div>
        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">取消</Button>
          </DialogClose>
          <Button onClick={handleSubmit} disabled={create.isPending || !name.trim() || !path.trim()}>
            创建
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 2: WorkspaceCard**

`frontend/src/components/workspace/WorkspaceCard.tsx`：

```tsx
import { useNavigate } from "react-router-dom";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { Workspace } from "@/types/api";
import { useDeleteWorkspace } from "@/hooks/useWorkspaces";

export function WorkspaceCard({ workspace }: { workspace: Workspace }) {
  const navigate = useNavigate();
  const del = useDeleteWorkspace();

  const handleDelete = () => {
    if (confirm(`删除工作区「${workspace.name}」及其所有 target 和 todo？`)) {
      del.mutate(workspace.id);
    }
  };

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">{workspace.name}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-2 text-sm">
        <div className="text-muted-foreground font-mono text-xs truncate">{workspace.path}</div>
        <div className="flex gap-2 pt-2">
          <Button size="sm" onClick={() => navigate(`/workspaces/${workspace.id}`)}>
            进入
          </Button>
          <Button size="sm" variant="outline" onClick={handleDelete} disabled={del.isPending}>
            删除
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
```

- [ ] **Step 3: WorkspacesPage**

替换 `frontend/src/pages/WorkspacesPage.tsx`：

```tsx
import { useWorkspaces } from "@/hooks/useWorkspaces";
import { WorkspaceCard } from "@/components/workspace/WorkspaceCard";
import { CreateWorkspaceDialog } from "@/components/workspace/CreateWorkspaceDialog";

export function WorkspacesPage() {
  const { data: workspaces, isLoading, isError } = useWorkspaces();

  return (
    <div className="p-6">
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-bold">工作区</h1>
        <CreateWorkspaceDialog />
      </div>

      {isLoading && <p className="text-muted-foreground">加载中…</p>}
      {isError && <p className="text-destructive">加载失败</p>}
      {workspaces && workspaces.length === 0 && (
        <p className="text-muted-foreground">还没有工作区，点「新建工作区」创建一个吧。</p>
      )}
      {workspaces && workspaces.length > 0 && (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {workspaces.map((ws) => (
            <WorkspaceCard key={ws.id} workspace={ws} />
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: typecheck + build**

```powershell
cd frontend
npm run typecheck
npm run build
```

- [ ] **Step 5: Commit**

```powershell
cd E:\vibe-dashboard
git add frontend
git commit -m "feat(frontend): implement WorkspacesPage with cards and create dialog"
```

---

## Task 11: WorkspaceViewPage 看板视图

**Files:**
- Modify: `frontend/src/pages/WorkspaceViewPage.tsx`
- Create: `frontend/src/components/target/TargetList.tsx`
- Create: `frontend/src/components/target/CreateTargetDialog.tsx`
- Create: `frontend/src/components/board/Board.tsx`
- Create: `frontend/src/components/board/BoardColumn.tsx`
- Create: `frontend/src/components/board/TodoCard.tsx`
- Create: `frontend/src/components/board/TodoDialog.tsx`

**Interfaces:**
- Produces: 看板视图（target 侧栏含"全部"选项 + 4 列看板 + todo 新建/编辑/改状态/删除）

- [ ] **Step 1: CreateTargetDialog**

`frontend/src/components/target/CreateTargetDialog.tsx`（参照 CreateWorkspaceDialog，传 workspaceId）：

```tsx
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useCreateTarget } from "@/hooks/useTargets";

export function CreateTargetDialog({ workspaceId }: { workspaceId: string }) {
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const create = useCreateTarget(workspaceId);

  const handleSubmit = async () => {
    if (!title.trim()) return;
    await create.mutateAsync({ title: title.trim(), description: description.trim() });
    setTitle("");
    setDescription("");
    setOpen(false);
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button size="sm" variant="outline">新建 Target</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>新建 Target</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 py-2">
          <div className="space-y-2">
            <Label htmlFor="t-title">标题</Label>
            <Input id="t-title" value={title} onChange={(e) => setTitle(e.target.value)} />
          </div>
          <div className="space-y-2">
            <Label htmlFor="t-desc">描述</Label>
            <Textarea id="t-desc" value={description} onChange={(e) => setDescription(e.target.value)} />
          </div>
        </div>
        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">取消</Button>
          </DialogClose>
          <Button onClick={handleSubmit} disabled={create.isPending || !title.trim()}>
            创建
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 2: TargetList（侧栏，含"全部"）**

`frontend/src/components/target/TargetList.tsx`：

```tsx
import { useTargets } from "@/hooks/useTargets";
import { CreateTargetDialog } from "./CreateTargetDialog";

interface Props {
  workspaceId: string;
  selectedTargetId: string | null;
  onSelect: (id: string | null) => void;
}

export function TargetList({ workspaceId, selectedTargetId, onSelect }: Props) {
  const { data: targets, isLoading } = useTargets(workspaceId);

  return (
    <div className="w-56 border-r bg-card p-4 space-y-2">
      <div className="flex items-center justify-between mb-2">
        <h3 className="text-sm font-semibold">Targets</h3>
        <CreateTargetDialog workspaceId={workspaceId} />
      </div>
      <button
        onClick={() => onSelect(null)}
        className={`block w-full text-left rounded-md px-2 py-1.5 text-sm hover:bg-accent ${
          selectedTargetId === null ? "bg-accent font-medium" : ""
        }`}
      >
        全部
      </button>
      {isLoading && <p className="text-xs text-muted-foreground">加载中…</p>}
      {targets?.map((t) => (
        <button
          key={t.id}
          onClick={() => onSelect(t.id)}
          className={`block w-full text-left rounded-md px-2 py-1.5 text-sm hover:bg-accent truncate ${
            selectedTargetId === t.id ? "bg-accent font-medium" : ""
          }`}
        >
          {t.title}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 3: TodoDialog（新建/编辑）**

`frontend/src/components/board/TodoDialog.tsx`：

```tsx
import { useState, useEffect } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useCreateTodo, useUpdateTodo } from "@/hooks/useTodos";
import type { Target, Todo, TodoStatus } from "@/types/api";

const TODO_STATUSES: TodoStatus[] = ["todo", "doing", "done", "blocked"];
const STATUS_LABEL: Record<TodoStatus, string> = {
  todo: "待办",
  doing: "进行中",
  done: "已完成",
  blocked: "阻塞",
};

interface Props {
  workspaceId: string;
  targets: Target[];
  editing?: Todo | null;
  defaultTargetId?: string;
  trigger: React.ReactNode;
}

export function TodoDialog({ workspaceId, targets, editing, defaultTargetId, trigger }: Props) {
  const [open, setOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [status, setStatus] = useState<TodoStatus>("todo");
  const [targetId, setTargetId] = useState(defaultTargetId ?? targets[0]?.id ?? "");

  const create = useCreateTodo(workspaceId);
  const update = useUpdateTodo(workspaceId);

  useEffect(() => {
    if (open) {
      setTitle(editing?.title ?? "");
      setDescription(editing?.description ?? "");
      setStatus(editing?.status ?? "todo");
      setTargetId(editing?.target_id ?? defaultTargetId ?? targets[0]?.id ?? "");
    }
  }, [open, editing, defaultTargetId, targets]);

  const handleSubmit = async () => {
    if (!title.trim() || !targetId) return;
    if (editing) {
      await update.mutateAsync({
        id: editing.id,
        input: { title: title.trim(), description: description.trim(), status },
      });
    } else {
      await create.mutateAsync({
        targetId,
        input: { title: title.trim(), description: description.trim() },
      });
    }
    setOpen(false);
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>{trigger}</DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{editing ? "编辑 Todo" : "新建 Todo"}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4 py-2">
          <div className="space-y-2">
            <Label htmlFor="todo-title">标题</Label>
            <Input id="todo-title" value={title} onChange={(e) => setTitle(e.target.value)} />
          </div>
          <div className="space-y-2">
            <Label htmlFor="todo-desc">描述</Label>
            <Textarea id="todo-desc" value={description} onChange={(e) => setDescription(e.target.value)} />
          </div>
          <div className="space-y-2">
            <Label>Target</Label>
            <Select value={targetId} onValueChange={setTargetId} disabled={!!editing}>
              {targets.map((t) => (
                <SelectItem key={t.id} value={t.id}>{t.title}</SelectItem>
              ))}
            </Select>
          </div>
          {editing && (
            <div className="space-y-2">
              <Label>状态</Label>
              <Select value={status} onValueChange={(v) => setStatus(v as TodoStatus)}>
                {TODO_STATUSES.map((s) => (
                  <SelectItem key={s} value={s}>{STATUS_LABEL[s]}</SelectItem>
                ))}
              </Select>
            </div>
          )}
        </div>
        <DialogFooter>
          <DialogClose asChild>
            <Button variant="outline">取消</Button>
          </DialogClose>
          <Button
            onClick={handleSubmit}
            disabled={create.isPending || update.isPending || !title.trim() || !targetId}
          >
            {editing ? "保存" : "创建"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 4: TodoCard**

`frontend/src/components/board/TodoCard.tsx`：

```tsx
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useUpdateTodo, useDeleteTodo } from "@/hooks/useTodos";
import type { Target, Todo, TodoStatus } from "@/types/api";
import { TodoDialog } from "./TodoDialog";

const TODO_STATUSES: TodoStatus[] = ["todo", "doing", "done", "blocked"];
const STATUS_LABEL: Record<TodoStatus, string> = {
  todo: "待办",
  doing: "进行中",
  done: "已完成",
  blocked: "阻塞",
};

interface Props {
  workspaceId: string;
  todo: Todo;
  targets: Target[];
}

export function TodoCard({ workspaceId, todo, targets }: Props) {
  const update = useUpdateTodo(workspaceId);
  const del = useDeleteTodo(workspaceId);
  const target = targets.find((t) => t.id === todo.target_id);

  return (
    <div className="rounded-md border bg-background p-3 space-y-2">
      <div className="flex items-start justify-between gap-2">
        <span className="text-sm font-medium">{todo.title}</span>
        {target && <Badge variant="secondary" className="text-xs shrink-0">{target.title}</Badge>}
      </div>
      {todo.description && (
        <p className="text-xs text-muted-foreground line-clamp-2">{todo.description}</p>
      )}
      <div className="flex items-center gap-2">
        <Select
          value={todo.status}
          onValueChange={(v) => update.mutate({ id: todo.id, input: { status: v as TodoStatus } })}
        >
          <SelectTrigger className="h-7 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {TODO_STATUSES.map((s) => (
              <SelectItem key={s} value={s}>{STATUS_LABEL[s]}</SelectItem>
            ))}
          </SelectContent>
        </Select>
        <TodoDialog
          workspaceId={workspaceId}
          targets={targets}
          editing={todo}
          trigger={<Button size="sm" variant="ghost" className="h-7 px-2 text-xs">编辑</Button>}
        />
        <Button
          size="sm"
          variant="ghost"
          className="h-7 px-2 text-xs text-destructive"
          onClick={() => del.mutate(todo.id)}
        >
          删除
        </Button>
      </div>
    </div>
  );
}
```

- [ ] **Step 5: BoardColumn + Board**

`frontend/src/components/board/BoardColumn.tsx`：

```tsx
import type { Todo, Target } from "@/types/api";
import { TodoCard } from "./TodoCard";

interface Props {
  title: string;
  status: string;
  todos: Todo[];
  workspaceId: string;
  targets: Target[];
}

export function BoardColumn({ title, todos, workspaceId, targets }: Props) {
  return (
    <div className="flex-1 min-w-[220px] rounded-md bg-muted/40 p-3">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-sm font-semibold">{title}</h3>
        <span className="text-xs text-muted-foreground">{todos.length}</span>
      </div>
      <div className="space-y-2">
        {todos.map((todo) => (
          <TodoCard key={todo.id} workspaceId={workspaceId} todo={todo} targets={targets} />
        ))}
        {todos.length === 0 && (
          <p className="text-xs text-muted-foreground py-4 text-center">无</p>
        )}
      </div>
    </div>
  );
}
```

`frontend/src/components/board/Board.tsx`：

```tsx
import { useMemo } from "react";
import { Button } from "@/components/ui/button";
import { useTodos } from "@/hooks/useTodos";
import type { Target, TodoStatus } from "@/types/api";
import { BoardColumn } from "./BoardColumn";
import { TodoDialog } from "./TodoDialog";

const COLUMNS: { status: TodoStatus; title: string }[] = [
  { status: "todo", title: "待办" },
  { status: "doing", title: "进行中" },
  { status: "done", title: "已完成" },
  { status: "blocked", title: "阻塞" },
];

interface Props {
  workspaceId: string;
  targets: Target[];
  selectedTargetId: string | null;
}

export function Board({ workspaceId, targets, selectedTargetId }: Props) {
  const { data: todos } = useTodos(workspaceId);

  const grouped = useMemo(() => {
    const filtered = selectedTargetId
      ? todos?.filter((t) => t.target_id === selectedTargetId)
      : todos;
    const map: Record<TodoStatus, typeof filtered> = {
      todo: [],
      doing: [],
      done: [],
      blocked: [],
    };
    filtered?.forEach((t) => map[t.status].push(t));
    return map;
  }, [todos, selectedTargetId]);

  return (
    <div className="flex-1 p-6 overflow-hidden flex flex-col">
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-2xl font-bold">看板</h1>
        {targets.length > 0 && (
          <TodoDialog
            workspaceId={workspaceId}
            targets={targets}
            defaultTargetId={selectedTargetId ?? undefined}
            trigger={<Button size="sm">新建 Todo</Button>}
          />
        )}
      </div>
      <div className="flex gap-4 overflow-x-auto flex-1">
        {COLUMNS.map((col) => (
          <BoardColumn
            key={col.status}
            title={col.title}
            status={col.status}
            todos={grouped[col.status] ?? []}
            workspaceId={workspaceId}
            targets={targets}
          />
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 6: WorkspaceViewPage 组装**

替换 `frontend/src/pages/WorkspaceViewPage.tsx`：

```tsx
import { useState } from "react";
import { useTargets } from "@/hooks/useTargets";
import { TargetList } from "@/components/target/TargetList";
import { Board } from "@/components/board/Board";

export function WorkspaceViewPage() {
  const { wid } = useParams();
  const [selectedTargetId, setSelectedTargetId] = useState<string | null>(null);
  const { data: targets } = useTargets(wid!);

  if (!wid) return null;

  return (
    <div className="flex flex-1">
      <TargetList
        workspaceId={wid}
        selectedTargetId={selectedTargetId}
        onSelect={setSelectedTargetId}
      />
      <Board workspaceId={wid} targets={targets ?? []} selectedTargetId={selectedTargetId} />
    </div>
  );
}
```

> 需 `import { useParams } from "react-router-dom";`。

- [ ] **Step 7: typecheck + build**

```powershell
cd frontend
npm run typecheck
npm run build
```

- [ ] **Step 8: Commit**

```powershell
cd E:\vibe-dashboard
git add frontend
git commit -m "feat(frontend): implement board view with target filter and todo CRUD"
```

---

## Task 12: 端到端联调 + 最终质量门禁

**Files:**
- 无新文件，全流程验证

- [ ] **Step 1: 后端启动**

```powershell
cd backend
$env:VIBE_DB_PATH = "E:\vibe-dashboard\backend\dev.db"
cargo run -p api
```

- [ ] **Step 2: 前端启动**

另开终端：

```powershell
cd frontend
npm run dev
```

- [ ] **Step 3: 浏览器验收（http://localhost:5173）**

按 spec 验收标准逐项走：
- [ ] `/` 看到 workspace 列表（空）
- [ ] 新建 workspace -> 自动跳转看板视图
- [ ] Sidebar 底部两个圆点：后端绿、ws 绿（后端在跑）
- [ ] 新建 target -> 出现在侧栏
- [ ] 新建 todo（选 target）-> 出现在"待办"列
- [ ] 卡片状态下拉改成"进行中" -> 卡片移到对应列
- [ ] 点编辑改标题/描述 -> 保存后更新
- [ ] 删除 todo -> 消失
- [ ] 侧栏选"全部" -> 看板显示所有 target 的 todo；选某 target -> 只显示该 target 的
- [ ] 回 `/`，删除 workspace -> 列表消失，进看板页应 404 友好处理（可选：看板页对不存在的 wid 显示提示）
- [ ] 杀后端 -> Sidebar 后端圆点变红、ws 圆点变红；重启后端 -> 恢复绿

- [ ] **Step 4: 后端最终门禁**

```powershell
cd backend
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
$env:DATABASE_URL = "sqlite:./dev.db"
$env:SQLX_OFFLINE = "false"
cargo sqlx prepare --workspace
$env:SQLX_OFFLINE = "true"
$env:SQLX_OFFLINE = "true"; cargo build
```

Expected: fmt/clippy/test 全过，`.sqlx/` 更新，离线编译通过。

- [ ] **Step 5: 前端最终门禁**

```powershell
cd frontend
npm run typecheck
npm run build
npm run lint
```

Expected: 全过。

- [ ] **Step 6: Commit 最终 .sqlx 缓存（若有变更）**

```powershell
cd E:\vibe-dashboard
git status
# 若 .sqlx 有变更：
git add backend/.sqlx
git commit -m "chore(db): update sqlx offline cache for L2"
```

---

## Self-Review

**1. Spec coverage**：
- ✅ 三实体 schema（migration 0002）-- Task 2
- ✅ `crates/tasks` 模型 + repo（`query!` 宏）-- Task 2-5
- ✅ REST CRUD（workspace/target/todo）-- Task 6-7
- ✅ 级联删除 -- Task 5 测试
- ✅ status CHECK + Rust 枚举校验 -- Task 2 models + Task 5
- ✅ `AppError` 下沉 shared -- Task 1
- ✅ `cargo sqlx prepare` 工作流 -- Task 3/4/5/7/12
- ✅ 临时文件库测试 -- Task 3-5
- ✅ 前端路由 -- Task 9
- ✅ Sidebar 底部 health/ws 圆点 -- Task 9
- ✅ 看板默认全部 + target 过滤 -- Task 11
- ✅ 4 列看板 + 状态下拉 -- Task 11
- ✅ HomePage 删除，全局状态 hook -- Task 9

**2. Placeholder scan**：
- Task 9 的 WorkspacesPage/WorkspaceViewPage 是占位，但 Task 10/11 立即填实，无遗留 TODO。
- Task 7 Step 4-6 的 api handler 测试标了实现者需补全非法 status 测试 -- 已在 Self-Review 标注。

**3. Type consistency**：
- `AppError` 从 shared 来，tasks/api 共享 -- Task 1
- 前后端字段名 snake_case 一致（Workspace/Target/Todo）-- Task 2/8
- status 枚举前后端值一致（`"planned"`/`"todo"` 等）-- Task 2/8
- `del` 处理 204 -- Task 8 Step 3

**4. 风险点**：
- Task 7 的 api crate 重构为 lib+bin 是较大改动，需仔细处理 `mod` 声明迁移。若出问题，退路是 handler 测试改为集成测试通过 HTTP 调用而非 oneshot。
- shadcn 组件 add 可能遇 React 19 peer dep 警告 -- Task 8 Step 1 已注。
- `query_as!` 对 `status: String` 字段：DB 列是 TEXT，宏推断为 `String`，与 model 字段类型匹配。若 sqlx 推断为 `Option<String>`（因 CHECK 约束不影响 nullability），需确认列是 `NOT NULL`（schema 已设）。
