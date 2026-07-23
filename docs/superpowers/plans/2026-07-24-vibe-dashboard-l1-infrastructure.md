# Vibe Dashboard L1 基础设施层 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建 vibe-dashboard 的基础设施层（L1）：可启动的 Rust 后端（Axum + SQLx + SQLite）、可启动的 Vite 前端（React + TS + shadcn/ui），二者通过 `/api/health` REST 和 `/ws` WebSocket 双向通道连通。

**Architecture:** Rust 用 cargo workspace 多 crate（`api`/`db`/`shared`）为后续 L2-L5 留位置。后端 axum 监听 `127.0.0.1:8787`，SQLite 存 `%APPDATA%\vibe-dashboard\data.db`（WAL 模式）。前端 Vite dev server 监听 5173，proxy `/api` 和 `/ws` 到后端。WS 用 Hub 模式（`Arc<Hub>` + `DashMap<ConnId, Sender>`）管理多连接，L1 只支持 hello/ping/pong 消息。

**Tech Stack:** Rust 1.97, Axum, tokio, SQLx (SQLite), tracing, tower-http, dashmap, serde; Node 25, Vite, React 18, TypeScript, TanStack Query, Zustand, shadcn/ui, Tailwind CSS.

## Global Constraints

- 后端端口固定 `8787`，前端 dev 端口固定 `5173`。
- SQLite 路径默认 `%APPDATA%\vibe-dashboard\data.db`（Windows），可被环境变量 `VIBE_DB_PATH` 覆盖。
- 配置走环境变量（`VIBE_DB_PATH` / `VIBE_HTTP_PORT` / `VIBE_LOG_LEVEL`），L1 不引入配置文件。
- 日志用 `tracing` + `tracing-subscriber` JSON 格式输出 stdout。
- WebSocket 消息 JSON 格式：`{"type": "<name>", "payload": {...}}`。
- 不做任何业务实体（Workspace/Target/To-Do）-- 留给 L2。
- 所有代码不加注释（除非用户要求）。
- 每个任务结束前必须 `cargo fmt --all` 和 `cargo clippy --all-targets -- -D warnings`（后端）或 `npm run typecheck`（前端）通过。
- 数据库 migration 用 `sqlx::migrate!` 宏运行时自动执行。
- SQLx 用 `query!` 宏做编译期校验，需要 `DATABASE_URL` 环境变量指向 dev 数据库；CI 用 `cargo sqlx prepare` 生成的 `.sqlx/` 缓存。
- 平台：Windows（PowerShell 5.1）。命令以 PowerShell 语法给出，但 cargo/npm 命令跨平台通用。

---

## File Structure

后端将创建/修改的文件：

| 文件 | 责任 |
|---|---|
| `backend/Cargo.toml` | workspace 根，声明 members 和共享依赖版本 |
| `backend/rust-toolchain.toml` | 固定 Rust 版本（可选但推荐） |
| `backend/.cargo/config.toml` | 设置 `DATABASE_URL`（sqlx 编译期校验用，相对路径 sqlite 文件） |
| `backend/.env` | dev 期 `DATABASE_URL`（gitignore） |
| `backend/.gitignore` | 忽略 target/、.env、*.db |
| `backend/crates/api/Cargo.toml` | api crate 清单 |
| `backend/crates/api/src/main.rs` | tokio 入口，组装 router、启动 server、优雅退出 |
| `backend/crates/api/src/config.rs` | `Config` struct + 从 env 加载 |
| `backend/crates/api/src/state.rs` | `AppState`（db pool、hub、config） |
| `backend/crates/api/src/error.rs` | `AppError` enum + `IntoResponse` |
| `backend/crates/api/src/routes/mod.rs` | 路由聚合 |
| `backend/crates/api/src/routes/health.rs` | `GET /api/health` |
| `backend/crates/api/src/routes/ws.rs` | `GET /ws` 升级处理 |
| `backend/crates/api/src/ws/mod.rs` | WS 模块入口 |
| `backend/crates/api/src/ws/hub.rs` | `Hub` 连接管理 + broadcast |
| `backend/crates/api/src/ws/session.rs` | 单连接的读循环 + 写循环 |
| `backend/crates/api/src/ws/message.rs` | WS 消息类型（`ClientMsg`/`ServerMsg`） |
| `backend/crates/db/Cargo.toml` | db crate 清单 |
| `backend/crates/db/src/lib.rs` | 暴露 `init_pool`、`run_migrations` |
| `backend/crates/db/src/pool.rs` | `SqlitePool` 初始化 + PRAGMA 设置 |
| `backend/crates/db/migrations/0001_init.sql` | L1 元数据表 |
| `backend/crates/shared/Cargo.toml` | shared crate 清单 |
| `backend/crates/shared/src/lib.rs` | re-export |
| `backend/crates/shared/src/logging.rs` | `tracing_subscriber` JSON 初始化 |
| `backend/.sqlx/` | sqlx 编译期校验缓存（由 `cargo sqlx prepare` 生成，commit 入库） |

前端将创建/修改的文件：

| 文件 | 责任 |
|---|---|
| `frontend/package.json` | 依赖与脚本 |
| `frontend/vite.config.ts` | dev server + proxy |
| `frontend/tsconfig.json` | TS 配置 |
| `frontend/tsconfig.node.json` | vite 配置文件的 TS 配置 |
| `frontend/index.html` | HTML 入口 |
| `frontend/postcss.config.js` | Tailwind + autoprefixer |
| `frontend/tailwind.config.js` | Tailwind 配置 |
| `frontend/components.json` | shadcn/ui 配置 |
| `frontend/src/main.tsx` | React 挂载 + Provider |
| `frontend/src/App.tsx` | 根组件 + 布局 |
| `frontend/src/index.css` | Tailwind 指令 + shadcn 变量 |
| `frontend/src/lib/api.ts` | fetch 封装 |
| `frontend/src/lib/ws.ts` | WS 客户端（重连、订阅） |
| `frontend/src/lib/query.ts` | TanStack Query client |
| `frontend/src/lib/utils.ts` | shadcn/ui 需要的 `cn()` |
| `frontend/src/stores/ui.ts` | Zustand UI store |
| `frontend/src/components/ui/card.tsx` | shadcn Card（用 CLI 生成） |
| `frontend/src/components/ui/button.tsx` | shadcn Button |
| `frontend/src/components/ui/badge.tsx` | shadcn Badge |
| `frontend/src/components/layout/Sidebar.tsx` | 占位侧边栏 |
| `frontend/src/pages/HomePage.tsx` | 占位首页（health + WS 状态） |

根级：

| 文件 | 责任 |
|---|---|
| `.gitignore` | 忽略 backend/target、frontend/node_modules、frontend/dist、.env、*.db 等 |
| `README.md` | 启动说明 |

---

## Task 1: 初始化 git 仓库与根目录脚手架

**Files:**
- Create: `E:\vibe-dashboard\.gitignore`
- Create: `E:\vibe-dashboard\README.md`

**Interfaces:**
- Consumes: 无
- Produces: 一个 git 仓库 + 根级忽略规则

- [ ] **Step 1: 初始化 git 仓库**

```powershell
cd E:\vibe-dashboard
git init
git config user.name | Out-Null; if (-not $?) { git config user.email | Out-Null }
```

> 若已有 `.git` 可跳过 init。后续步骤假设 `E:\vibe-dashboard` 是仓库根。

- [ ] **Step 2: 创建根 `.gitignore`**

文件 `E:\vibe-dashboard\.gitignore`：

```gitignore
# Rust
backend/target/
backend/**/*.rs.bk

# SQLx 编译期缓存保留（不忽略 backend/.sqlx/）

# Node
frontend/node_modules/
frontend/dist/
frontend/.vite/

# 环境与本地数据
.env
.env.local
*.db
*.db-journal
*.db-wal
*.db-shm

# IDE
.vscode/
.idea/
*.swp
.DS_Store
Thumbs.db

# OS
~$*
```

- [ ] **Step 3: 创建根 `README.md`**

文件 `E:\vibe-dashboard\README.md`：

```markdown
# Vibe Dashboard

本地单用户的 AI 编程管理工具。

## 开发启动

### 后端（端口 8787）

```powershell
cd backend
cargo run -p api
```

### 前端（端口 5173）

```powershell
cd frontend
npm install
npm run dev
```

浏览器打开 http://localhost:5173

## 技术栈

- 后端：Rust + Axum + SQLx + SQLite
- 前端：Vite + React + TypeScript + shadcn/ui
```

- [ ] **Step 4: 首次提交**

```powershell
cd E:\vibe-dashboard
git add .gitignore README.md docs
git commit -m "chore: init repo with gitignore and README"
```

---

## Task 2: 创建 Rust cargo workspace 骨架与依赖锁定

**Files:**
- Create: `backend/Cargo.toml`
- Create: `backend/rust-toolchain.toml`
- Create: `backend/.cargo/config.toml`
- Create: `backend/.env`
- Create: `backend/.gitignore`
- Create: `backend/crates/api/Cargo.toml`
- Create: `backend/crates/api/src/main.rs`（占位，仅编译通过）
- Create: `backend/crates/db/Cargo.toml`
- Create: `backend/crates/db/src/lib.rs`（占位）
- Create: `backend/crates/shared/Cargo.toml`
- Create: `backend/crates/shared/src/lib.rs`（占位）

**Interfaces:**
- Consumes: 无
- Produces: `cargo build` 在 workspace 根成功；三个 crate 互相依赖关系建立（api 依赖 db、shared；db 依赖 shared）

- [ ] **Step 1: 创建 workspace 根 `Cargo.toml`**

文件 `backend/Cargo.toml`：

```toml
[workspace]
resolver = "2"
members = ["crates/api", "crates/db", "crates/shared"]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.97"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
axum = { version = "0.7", features = ["ws", "macros"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "fs", "cors"] }
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "macros", "migrate", "chrono"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
dashmap = "6"
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "1"
anyhow = "1"
async-trait = "0.1"
futures = "0.3"
tokio-stream = "0.1"

[profile.dev]
debug = 1

[profile.release]
debug = 0
lto = "thin"
```

- [ ] **Step 2: 创建 `rust-toolchain.toml` 固定版本**

文件 `backend/rust-toolchain.toml`：

```toml
[toolchain]
channel = "1.97.0"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 3: 创建 `.cargo/config.toml` 配置 sqlx 编译期校验的 DATABASE_URL**

文件 `backend/.cargo/config.toml`：

```toml
[env]
DATABASE_URL = "sqlite:./dev.db"
SQLX_OFFLINE = "true"
```

> `SQLX_OFFLINE=true` 让编译期用 `.sqlx/` 缓存而非连库；首次 prepare 前会失败，后续任务会处理。

- [ ] **Step 4: 创建 `.env`（dev 期连库用，gitignore）**

文件 `backend/.env`：

```env
DATABASE_URL=sqlite:./dev.db
VIBE_DB_PATH=
VIBE_HTTP_PORT=8787
VIBE_LOG_LEVEL=info
```

- [ ] **Step 5: 创建 backend 级 `.gitignore`**

文件 `backend/.gitignore`：

```gitignore
/target
dev.db
dev.db-journal
dev.db-wal
dev.db-shm
.env
```

- [ ] **Step 6: 创建 `crates/shared` 骨架**

文件 `backend/crates/shared/Cargo.toml`：

```toml
[package]
name = "shared"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
tracing.workspace = true
tracing-subscriber.workspace = true
```

文件 `backend/crates/shared/src/lib.rs`：

```rust
pub mod logging;
```

文件 `backend/crates/shared/src/logging.rs`（占位，下个 task 实现）：

```rust
pub fn init(_level: &str) {}
```

- [ ] **Step 7: 创建 `crates/db` 骨架**

文件 `backend/crates/db/Cargo.toml`：

```toml
[package]
name = "db"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
shared = { path = "../shared" }
sqlx.workspace = true
tokio.workspace = true
tracing.workspace = true
thiserror.workspace = true
```

文件 `backend/crates/db/src/lib.rs`：

```rust
pub mod pool;
```

文件 `backend/crates/db/src/pool.rs`（占位）：

```rust
pub async fn init_pool(_url: &str) -> sqlx::Result<sqlx::SqlitePool> {
    unimplemented!()
}
```

- [ ] **Step 8: 创建 `crates/api` 骨架**

文件 `backend/crates/api/Cargo.toml`：

```toml
[package]
name = "api"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
shared = { path = "../shared" }
db = { path = "../db" }
tokio.workspace = true
axum.workspace = true
tower.workspace = true
tower-http.workspace = true
sqlx.workspace = true
serde.workspace = true
serde_json.workspace = true
chrono.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
dashmap.workspace = true
uuid.workspace = true
thiserror.workspace = true
anyhow.workspace = true
futures.workspace = true
tokio-stream.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

文件 `backend/crates/api/src/main.rs`（占位，仅编译通过）：

```rust
fn main() {
    println!("api placeholder");
}
```

- [ ] **Step 9: 验证 workspace 编译**

```powershell
cd backend
cargo build
```

Expected: 编译成功，可能有 unused warning（占位代码），下个任务会清理。

- [ ] **Step 10: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/Cargo.toml backend/rust-toolchain.toml backend/.cargo backend/.gitignore backend/crates
git commit -m "chore: scaffold rust workspace with api/db/shared crates"
```

---

## Task 3: 安装 sqlx-cli 并实现 shared::logging

**Files:**
- Modify: `backend/crates/shared/src/logging.rs`

**Interfaces:**
- Consumes: 无
- Produces: `shared::logging::init(level: &str)` 初始化全局 tracing subscriber（JSON 格式，env-filter）

- [ ] **Step 1: 安装 sqlx-cli（含 sqlite 支持）**

```powershell
cargo install sqlx-cli --no-default-features --features sqlite,rustls
```

Expected: 安装完成，`cargo sqlx --version` 输出版本号。

> 这一步耗时较长（编译 sqlx-cli）。后续 `cargo sqlx prepare` 依赖它。

- [ ] **Step 2: 写 logging 单元测试（验证 init 不 panic）**

文件 `backend/crates/shared/src/logging.rs`，替换占位为：

```rust
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init(level: &str) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().json())
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_does_not_panic_with_valid_level() {
        init("info");
    }

    #[test]
    fn init_does_not_panic_with_debug_level() {
        init("debug");
    }
}
```

- [ ] **Step 3: 运行测试**

```powershell
cd backend
cargo test -p shared
```

Expected: 2 个测试通过。

> 注意：`tracing_subscriber::init` 在已初始化时会 panic，但测试中每个测试是独立进程内顺序运行且 subscriber 是全局的，第一个测试 init 后第二个会 panic。改用 `try_init` 更安全。

- [ ] **Step 4: 修正为 try_init 避免重复初始化 panic**

替换 `logging.rs` 全文为：

```rust
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init(level: &str) {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    let _ = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().json())
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_does_not_panic_with_valid_level() {
        init("info");
        init("debug");
    }
}
```

- [ ] **Step 5: 再次运行测试验证通过**

```powershell
cd backend
cargo test -p shared
```

Expected: 测试通过。

- [ ] **Step 6: 格式化 + clippy**

```powershell
cd backend
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Expected: 无 warning。

- [ ] **Step 7: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/crates/shared
git commit -m "feat(shared): implement json tracing init"
```

---

## Task 4: 实现 db crate 连接池与 migration

**Files:**
- Modify: `backend/crates/db/src/pool.rs`
- Modify: `backend/crates/db/src/lib.rs`
- Create: `backend/crates/db/migrations/0001_init.sql`
- Create: `backend/crates/db/tests/pool_test.rs`

**Interfaces:**
- Consumes: `shared::logging`
- Produces:
  - `db::init_pool(url: &str) -> SqlitePool`：创建连接池，设置 WAL + foreign_keys
  - `db::run_migrations(pool: &SqlitePool) -> Result<()>`：运行 `migrations/` 下 SQL
  - `db::DbError`：错误类型

- [ ] **Step 1: 创建 migration SQL**

文件 `backend/crates/db/migrations/0001_init.sql`：

```sql
CREATE TABLE IF NOT EXISTS schema_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT INTO schema_meta(key, value) VALUES('schema_version', '1')
    ON CONFLICT(key) DO UPDATE SET value = excluded.value;

INSERT INTO schema_meta(key, value) VALUES('created_at', strftime('%Y-%m-%dT%H:%M:%fZ','now'))
    ON CONFLICT(key) DO NOTHING;
```

- [ ] **Step 2: 写 db 测试（先失败）**

文件 `backend/crates/db/tests/pool_test.rs`：

```rust
use db::{init_pool, run_migrations};
use sqlx::sqlite::SqlitePoolOptions;

async fn setup_pool() -> sqlx::SqlitePool {
    let url = "sqlite::memory:";
    let pool = init_pool(url).await.expect("pool init failed");
    run_migrations(&pool).await.expect("migrations failed");
    pool
}

#[tokio::test]
async fn init_pool_creates_working_pool() {
    let pool = setup_pool().await;
    let (val,): (String,) = sqlx::query_as("SELECT 'ok' AS val")
        .fetch_one(&pool)
        .await
        .expect("query failed");
    assert_eq!(val, "ok");
}

#[tokio::test]
async fn migrations_create_schema_meta_table() {
    let pool = setup_pool().await;
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) AS count FROM schema_meta")
        .fetch_one(&pool)
        .await
        .expect("query failed");
    assert!(count >= 2, "schema_meta should have schema_version and created_at");
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let pool = setup_pool().await;
    run_migrations(&pool).await.expect("re-run migrations failed");
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) AS count FROM schema_meta")
        .fetch_one(&pool)
        .await
        .expect("query failed");
    assert_eq!(count, 2, "schema_meta should still have 2 rows after re-run");
}

#[tokio::test]
async fn wal_mode_enabled() {
    let pool = setup_pool().await;
    let (mode,): (String,) = sqlx::query_as("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .expect("pragma failed");
    assert_eq!(mode.to_lowercase(), "wal");
}
```

- [ ] **Step 3: 运行测试验证失败**

```powershell
cd backend
cargo test -p db
```

Expected: 编译失败（`init_pool` 和 `run_migrations` 未实现 / 签名不对）。

- [ ] **Step 4: 实现 `db::pool` 和 `lib.rs`**

文件 `backend/crates/db/src/pool.rs`：

```rust
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use std::str::FromStr;
use tracing::info;

pub async fn init_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = SqliteConnectOptions::from_str(url)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    info!(url = %url, "sqlite pool initialized");
    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    info!("migrations applied");
    Ok(())
}
```

文件 `backend/crates/db/src/lib.rs`：

```rust
pub mod pool;

pub use pool::{init_pool, run_migrations};
```

- [ ] **Step 5: 运行测试验证通过**

```powershell
cd backend
cargo test -p db
```

Expected: 4 个测试通过。

- [ ] **Step 6: 格式化 + clippy**

```powershell
cd backend
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Expected: 无 warning。

- [ ] **Step 7: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/crates/db
git commit -m "feat(db): implement sqlite pool with wal and migrations"
```

---

## Task 5: 实现 api crate 的 Config 与 AppError

**Files:**
- Create: `backend/crates/api/src/config.rs`
- Create: `backend/crates/api/src/error.rs`
- Create: `backend/crates/api/src/config_tests.rs`（或内联 #[cfg(test)]）
- Create: `backend/crates/api/src/error_tests.rs`（或内联）

**Interfaces:**
- Consumes: 无（仅标准库 + serde）
- Produces:
  - `api::config::Config { db_path, http_port, log_level }` + `Config::from_env() -> Result<Config, ConfigError>`
  - `api::error::AppError` enum + `impl IntoResponse`
  - `api::error::AppResult<T> = Result<T, AppError>`

- [ ] **Step 1: 写 Config 测试（先失败）**

创建文件 `backend/crates/api/src/config.rs`：

```rust
use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub db_path: String,
    pub http_port: u16,
    pub log_level: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("invalid port: {0}")]
    InvalidPort(String),
    #[error("env var error: {0}")]
    Env(#[from] env::VarError),
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let db_path = env::var("VIBE_DB_PATH")
            .unwrap_or_else(|_| default_db_path());
        let http_port: u16 = env::var("VIBE_HTTP_PORT")
            .unwrap_or_else(|_| "8787".to_string())
            .parse()
            .map_err(|e| ConfigError::InvalidPort(e.to_string()))?;
        let log_level = env::var("VIBE_LOG_LEVEL")
            .unwrap_or_else(|_| "info".to_string());
        Ok(Self { db_path, http_port, log_level })
    }
}

fn default_db_path() -> String {
    if let Some(appdata) = env::var("APPDATA").ok() {
        format!("{}\\vibe-dashboard\\data.db", appdata)
    } else {
        "data.db".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_env() {
        env::remove_var("VIBE_DB_PATH");
        env::remove_var("VIBE_HTTP_PORT");
        env::remove_var("VIBE_LOG_LEVEL");
        env::remove_var("APPDATA");
    }

    #[test]
    fn from_env_uses_defaults_when_unset() {
        clear_env();
        let cfg = Config::from_env().expect("defaults");
        assert_eq!(cfg.http_port, 8787);
        assert_eq!(cfg.log_level, "info");
        assert!(cfg.db_path.ends_with("data.db"));
    }

    #[test]
    fn from_env_reads_overrides() {
        clear_env();
        env::set_var("VIBE_DB_PATH", "/tmp/test.db");
        env::set_var("VIBE_HTTP_PORT", "9999");
        env::set_var("VIBE_LOG_LEVEL", "debug");
        let cfg = Config::from_env().expect("overrides");
        assert_eq!(cfg.db_path, "/tmp/test.db");
        assert_eq!(cfg.http_port, 9999);
        assert_eq!(cfg.log_level, "debug");
        clear_env();
    }

    #[test]
    fn from_env_rejects_invalid_port() {
        clear_env();
        env::set_var("VIBE_HTTP_PORT", "not-a-port");
        let err = Config::from_env().unwrap_err();
        assert!(matches!(err, ConfigError::InvalidPort(_)));
        clear_env();
    }

    #[test]
    fn default_db_path_uses_appdata_on_windows() {
        clear_env();
        env::set_var("APPDATA", "C:\\Users\\test\\AppData\\Roaming");
        let cfg = Config::from_env().expect("appdata");
        assert_eq!(cfg.db_path, "C:\\Users\\test\\AppData\\Roaming\\vibe-dashboard\\data.db");
        clear_env();
    }
}
```

- [ ] **Step 2: 写 AppError**

创建文件 `backend/crates/api/src/error.rs`：

```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

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

- [ ] **Step 3: 暴露模块入口**

修改 `backend/crates/api/src/main.rs`：

```rust
mod config;
mod error;

fn main() {
    println!("api placeholder");
}
```

- [ ] **Step 4: 运行测试**

```powershell
cd backend
cargo test -p api
```

Expected: Config 4 个测试 + AppError 3 个测试通过。

- [ ] **Step 5: 格式化 + clippy**

```powershell
cd backend
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Expected: 无 warning。

- [ ] **Step 6: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/crates/api/src/config.rs backend/crates/api/src/error.rs backend/crates/api/src/main.rs
git commit -m "feat(api): add Config and AppError"
```

---

## Task 6: 实现 WS 消息类型与 Hub

**Files:**
- Create: `backend/crates/api/src/ws/mod.rs`
- Create: `backend/crates/api/src/ws/message.rs`
- Create: `backend/crates/api/src/ws/hub.rs`
- Create: `backend/crates/api/src/ws/session.rs`

**Interfaces:**
- Consumes: `uuid`、`dashmap`、`tokio::sync`
- Produces:
  - `ws::message::{ClientMsg, ServerMsg}`：枚举 + serde
  - `ws::hub::Hub`：`Arc<Hub>`，`Hub::new() -> Self`、`Hub::register() -> ConnId`、`Hub::unregister(id)`、`Hub::send_to(id, msg)`、`Hub::broadcast(msg)`、`Hub::handle_client_msg(id, msg)`
  - `ws::session::ConnId = uuid::Uuid`

- [ ] **Step 1: 写消息类型**

文件 `backend/crates/api/src/ws/message.rs`：

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ClientMsg {
    Ping,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
pub enum ServerMsg {
    Hello {
        connection_id: Uuid,
        server_time: DateTime<Utc>,
    },
    Pong {
        server_time: DateTime<Utc>,
    },
}

impl ServerMsg {
    pub fn hello(connection_id: Uuid) -> Self {
        Self::Hello {
            connection_id,
            server_time: Utc::now(),
        }
    }

    pub fn pong() -> Self {
        Self::Pong {
            server_time: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_ping() {
        let json = r#"{"type":"ping"}"#;
        let msg: ClientMsg = serde_json::from_str(json).expect("parse");
        assert!(matches!(msg, ClientMsg::Ping));
    }

    #[test]
    fn serialize_hello() {
        let id = Uuid::new_v4();
        let msg = ServerMsg::hello(id);
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(json.contains("\"type\":\"hello\""));
        assert!(json.contains(&id.to_string()));
    }

    #[test]
    fn serialize_pong() {
        let msg = ServerMsg::pong();
        let json = serde_json::to_string(&msg).expect("serialize");
        assert!(json.contains("\"type\":\"pong\""));
    }
}
```

- [ ] **Step 2: 写 Hub（含内联测试）**

文件 `backend/crates/api/src/ws/hub.rs`：

```rust
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use super::message::ServerMsg;

pub type ConnId = Uuid;

pub struct Hub {
    senders: DashMap<ConnId, mpsc::UnboundedSender<ServerMsg>>,
}

impl Hub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            senders: DashMap::new(),
        })
    }

    pub fn register(&self) -> (ConnId, mpsc::UnboundedReceiver<ServerMsg>) {
        let id = Uuid::new_v4();
        let (tx, rx) = mpsc::unbounded_channel();
        self.senders.insert(id, tx);
        tracing::info!(conn_id = %id, "ws connection registered");
        (id, rx)
    }

    pub fn unregister(&self, id: ConnId) {
        if self.senders.remove(&id).is_some() {
            tracing::info!(conn_id = %id, "ws connection unregistered");
        }
    }

    pub fn send_to(&self, id: ConnId, msg: ServerMsg) -> bool {
        if let Some(tx) = self.senders.get(&id) {
            tx.send(msg).is_ok()
        } else {
            false
        }
    }

    pub fn broadcast(&self, msg: ServerMsg) {
        for entry in self.senders.iter() {
            let _ = entry.value().send(msg.clone());
        }
    }

    pub fn connection_count(&self) -> usize {
        self.senders.len()
    }

    pub fn handle_client_msg(&self, id: ConnId, msg: super::message::ClientMsg) {
        match msg {
            super::message::ClientMsg::Ping => {
                let _ = self.send_to(id, ServerMsg::pong());
            }
        }
    }
}

impl Default for Hub {
    fn default() -> Self {
        Arc::new(Self {
            senders: DashMap::new(),
        }).as_ref().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ws::message::ClientMsg;

    #[tokio::test]
    async fn register_returns_unique_ids() {
        let hub = Hub::new();
        let (id1, _rx1) = hub.register();
        let (id2, _rx2) = hub.register();
        assert_ne!(id1, id2);
        assert_eq!(hub.connection_count(), 2);
    }

    #[tokio::test]
    async fn unregister_removes_connection() {
        let hub = Hub::new();
        let (id, _rx) = hub.register();
        assert_eq!(hub.connection_count(), 1);
        hub.unregister(id);
        assert_eq!(hub.connection_count(), 0);
    }

    #[tokio::test]
    async fn send_to_delivers_message() {
        let hub = Hub::new();
        let (id, mut rx) = hub.register();
        let sent = hub.send_to(id, ServerMsg::pong());
        assert!(sent);
        let msg = rx.recv().await.expect("should receive");
        assert!(matches!(msg, ServerMsg::Pong { .. }));
    }

    #[tokio::test]
    async fn send_to_returns_false_for_unknown() {
        let hub = Hub::new();
        let sent = hub.send_to(Uuid::new_v4(), ServerMsg::pong());
        assert!(!sent);
    }

    #[tokio::test]
    async fn broadcast_reaches_all() {
        let hub = Hub::new();
        let (_id1, mut rx1) = hub.register();
        let (_id2, mut rx2) = hub.register();
        hub.broadcast(ServerMsg::pong());
        assert!(rx1.recv().await.is_some());
        assert!(rx2.recv().await.is_some());
    }

    #[tokio::test]
    async fn handle_ping_replies_pong() {
        let hub = Hub::new();
        let (id, mut rx) = hub.register();
        hub.handle_client_msg(id, ClientMsg::Ping);
        let msg = rx.recv().await.expect("should receive pong");
        assert!(matches!(msg, ServerMsg::Pong { .. }));
    }
}
```

- [ ] **Step 3: 写 session（连接读循环）**

文件 `backend/crates/api/src/ws/session.rs`：

```rust
use axum::extract::ws::{Message, WebSocket};
use futures::{sink::SinkExt, stream::StreamExt};
use std::time::Duration;
use tokio::time::{interval, Instant};
use uuid::Uuid;

use super::hub::Hub;
use super::message::{ClientMsg, ServerMsg};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn handle_connection(ws: WebSocket, hub: std::sync::Arc<Hub>) {
    let (id, mut rx) = hub.register();
    let (mut ws_sink, mut ws_stream) = ws.split();

    let hello = ServerMsg::hello(id);
    let hello_json = serde_json::to_string(&hello).expect("serialize hello");
    if ws_sink.send(Message::Text(hello_json)).await.is_err() {
        hub.unregister(id);
        return;
    }

    let send_task = tokio::spawn(async move {
        let mut ticker = interval(HEARTBEAT_INTERVAL);
        loop {
            tokio::select! {
                msg = rx.recv() => {
                    match msg {
                        Some(msg) => {
                            let json = serde_json::to_string(&msg).expect("serialize");
                            if ws_sink.send(Message::Text(json)).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                _ = ticker.tick() => {
                    if ws_sink.send(Message::Ping(Vec::new())).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    let recv_task = tokio::spawn(async move {
        let mut last_pong = Instant::now();
        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Text(text) => {
                    match serde_json::from_str::<ClientMsg>(&text) {
                        Ok(client_msg) => hub.handle_client_msg(id, client_msg),
                        Err(e) => {
                            tracing::warn!(conn_id = %id, error = %e, "invalid ws message");
                        }
                    }
                }
                Message::Pong(_) => {
                    last_pong = Instant::now();
                }
                Message::Close(_) => break,
                _ => {}
            }
            if last_pong.elapsed() > CLIENT_TIMEOUT {
                tracing::warn!(conn_id = %id, "ws client timeout, dropping");
                break;
            }
        }
        let _ = id;
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    hub.unregister(id);
}

#[allow(dead_code)]
fn _ensure_uuid_imported() -> Uuid {
    Uuid::new_v4()
}
```

> 说明：服务端每 30s 发 WebSocket 协议层 `Message::Ping`（非应用层 ping），浏览器自动回 `Message::Pong`。若 10s 内没收到 pong（用 `last_pong.elapsed() > CLIENT_TIMEOUT` 检测，配合 recv 循环每次迭代检查），断开连接。注意：浏览器自动回 pong，所以只要连接活着 `last_pong` 就会更新；若连接死亡，recv 也会返回 None 退出。这里的超时检测主要防御半开连接。

- [ ] **Step 4: 写 ws mod 入口**

文件 `backend/crates/api/src/ws/mod.rs`：

```rust
pub mod hub;
pub mod message;
pub mod session;

pub use hub::{Hub, ConnId};
```

- [ ] **Step 5: 更新 main.rs 暴露模块**

修改 `backend/crates/api/src/main.rs`：

```rust
mod config;
mod error;
mod ws;

fn main() {
    println!("api placeholder");
}
```

- [ ] **Step 6: 运行测试**

```powershell
cd backend
cargo test -p api
```

Expected: 消息类型 3 个 + Hub 6 个测试通过。

- [ ] **Step 7: 格式化 + clippy**

```powershell
cd backend
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Expected: 无 warning。

> 若 clippy 报 `Default` 实现返回 Arc 不符合惯例，删掉 `impl Default for Hub`（Hub 不需要 Default，调用方用 `Hub::new()`）。

- [ ] **Step 8: 删除冗余 Default impl**

若 clippy 通过则跳过；若警告则修改 `hub.rs` 删除：

```rust
impl Default for Hub {
    fn default() -> Self {
        Arc::new(Self {
            senders: DashMap::new(),
        }).as_ref().clone()
    }
}
```

只保留 `pub fn new() -> Arc<Self>`。

- [ ] **Step 9: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/crates/api/src/ws backend/crates/api/src/main.rs
git commit -m "feat(api): add ws message types, Hub, and session handler"
```

---

## Task 7: 实现 AppState、health 路由、ws 路由与 main 启动

**Files:**
- Create: `backend/crates/api/src/state.rs`
- Create: `backend/crates/api/src/routes/mod.rs`
- Create: `backend/crates/api/src/routes/health.rs`
- Create: `backend/crates/api/src/routes/ws.rs`
- Modify: `backend/crates/api/src/main.rs`

**Interfaces:**
- Consumes: `config::Config`、`error::AppError`、`ws::Hub`、`db::{init_pool, run_migrations}`、`shared::logging::init`
- Produces:
  - `api::state::AppState { db, hub, config, started_at }`
  - `GET /api/health` 返回 `{status, version, uptime_seconds}`
  - `GET /ws` 升级 WebSocket 并交给 `ws::session::handle_connection`
  - 可启动的 server（`cargo run -p api`）

- [ ] **Step 1: 写 AppState**

文件 `backend/crates/api/src/state.rs`：

```rust
use std::sync::Arc;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::config::Config;
use crate::ws::Hub;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub hub: Arc<Hub>,
    pub config: Arc<Config>,
    pub started_at: DateTime<Utc>,
}

impl AppState {
    pub fn new(db: SqlitePool, hub: Arc<Hub>, config: Config) -> Self {
        Self {
            db,
            hub,
            config: Arc::new(config),
            started_at: Utc::now(),
        }
    }
}
```

- [ ] **Step 2: 写 health 路由**

文件 `backend/crates/api/src/routes/health.rs`：

```rust
use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::AppResult;
use crate::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub uptime_seconds: f64,
}

pub async fn health(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    let uptime = (Utc::now() - state.started_at).num_milliseconds() as f64 / 1000.0;
    Ok(Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime,
    }))
}
```

> 注意 `health.rs` 用了 `Utc::now()`，需要在文件顶部加 `use chrono::Utc;`。修正后完整文件顶部 import：

```rust
use axum::extract::State;
use axum::Json;
use chrono::Utc;
use serde::Serialize;

use crate::error::AppResult;
use crate::state::AppState;
```

- [ ] **Step 3: 写 ws 路由**

文件 `backend/crates/api/src/routes/ws.rs`：

```rust
use axum::{
    extract::{
        ws::WebSocketUpgrade,
        State,
    },
    response::Response,
};
use std::sync::Arc;

use crate::state::AppState;
use crate::ws::{session::handle_connection, Hub};

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    let hub: Arc<Hub> = state.hub;
    ws.on_upgrade(move |socket| handle_connection(socket, hub))
}
```

- [ ] **Step 4: 写 routes mod**

文件 `backend/crates/api/src/routes/mod.rs`：

```rust
pub mod health;
pub mod ws;
```

- [ ] **Step 5: 写 main.rs 完整启动逻辑**

文件 `backend/crates/api/src/main.rs`（替换全部）：

```rust
mod config;
mod error;
mod routes;
mod state;
mod ws;

use std::sync::Arc;

use axum::{routing::get, Router};
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::config::Config;
use crate::state::AppState;
use crate::ws::Hub;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    shared::logging::init(&config.log_level);

    let db_url = format!("sqlite:{}", config.db_path);
    let pool = db::init_pool(&db_url).await?;
    db::run_migrations(&pool).await?;

    let hub = Hub::new();
    let state = AppState::new(pool, hub, config.clone());

    let app = build_router(state.clone());

    let addr = format!("127.0.0.1:{}", config.http_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(addr = %addr, "server starting");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("server stopped");
    Ok(())
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(routes::health::health))
        .route("/ws", get(routes::ws::ws_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed installing Ctrl-C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed installing signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }

    info!("shutdown signal received");
}
```

- [ ] **Step 6: 更新 api/Cargo.toml 添加 path 依赖（已在前序任务声明，确认 db 和 shared 已在依赖中）**

确认 `backend/crates/api/Cargo.toml` 的 `[dependencies]` 已包含 `shared` 和 `db`（Task 2 已加）。若没有则补：

```toml
shared = { path = "../shared" }
db = { path = "../db" }
```

- [ ] **Step 7: 设置 dev 数据库以便编译期 sqlx 校验**

由于 L1 的 `api` crate 暂未使用 `query!` 宏（health 不查库），编译不需要 `DATABASE_URL` 连库。但 `.cargo/config.toml` 已设 `DATABASE_URL=sqlite:./dev.db` 和 `SQLX_OFFLINE=true`，且 `.sqlx/` 目录还不存在。

为避免后续任务受阻，先建一个空的 `.sqlx/` 占位：

```powershell
cd backend
New-Item -ItemType Directory -Path ".sqlx" -Force | Out-Null
New-Item -ItemType File -Path ".sqlx/.gitkeep" -Force | Out-Null
```

- [ ] **Step 8: 编译验证**

```powershell
cd backend
cargo build
```

Expected: 编译成功，`api` 二进制生成在 `target/debug/api.exe`。

> 若报 `sqlx` 编译期连库错误，确认 `.cargo/config.toml` 中 `SQLX_OFFLINE=true`。L1 的 `api` crate 没用 `query!` 宏所以不会触发校验；后续 L2 才会。

- [ ] **Step 9: 手动启动验证**

第一个终端：

```powershell
cd backend
$env:VIBE_DB_PATH = "E:\vibe-dashboard\backend\dev.db"
cargo run -p api
```

Expected: 输出 `server starting addr=127.0.0.1:8787` 日志（JSON）。

第二个终端：

```powershell
curl http://127.0.0.1:8787/api/health
```

Expected: `{"status":"ok","version":"0.1.0","uptime_seconds":...}`

> `version` 来自 `env!("CARGO_PKG_VERSION")`，即 workspace.package.version = "0.1.0"。

回第一个终端 Ctrl+C 验证优雅退出（看到 `shutdown signal received` 和 `server stopped` 日志）。

- [ ] **Step 10: 格式化 + clippy**

```powershell
cd backend
cargo fmt --all
cargo clippy --all-targets -- -D warnings
```

Expected: 无 warning。

- [ ] **Step 11: Commit**

```powershell
cd E:\vibe-dashboard
git add backend/crates/api/src backend/.sqlx
git commit -m "feat(api): wire up appstate, health/ws routes, and server startup"
```

---

## Task 8: 前端脚手架（Vite + React + TS + Tailwind + shadcn/ui）

**Files:**
- Create: `frontend/package.json`
- Create: `frontend/vite.config.ts`
- Create: `frontend/tsconfig.json`
- Create: `frontend/tsconfig.node.json`
- Create: `frontend/index.html`
- Create: `frontend/postcss.config.js`
- Create: `frontend/tailwind.config.js`
- Create: `frontend/components.json`
- Create: `frontend/src/main.tsx`
- Create: `frontend/src/App.tsx`
- Create: `frontend/src/index.css`
- Create: `frontend/src/lib/utils.ts`
- Create: `frontend/src/vite-env.d.ts`

**Interfaces:**
- Consumes: 无
- Produces: `npm run dev` 启动 Vite dev server（5173），浏览器打开能看到空白 React 页；`npm run build` 构建成功；`npm run typecheck` 通过

- [ ] **Step 1: 创建 frontend 目录并用 Vite 初始化**

```powershell
cd E:\vibe-dashboard
npm create vite@latest frontend -- --template react-ts
```

> 若交互式询问，选 React + TypeScript。生成后 `frontend/package.json` 已存在。

- [ ] **Step 2: 安装基础依赖**

```powershell
cd frontend
npm install
```

- [ ] **Step 3: 安装 Tailwind + PostCSS + autoprefixer**

```powershell
cd frontend
npm install -D tailwindcss@3 postcss autoprefixer
```

> 用 Tailwind 3（v4 配置不同，shadcn/ui 当前稳定支持 v3）。

- [ ] **Step 4: 初始化 Tailwind 配置**

```powershell
cd frontend
npx tailwindcss init -p
```

Expected: 生成 `tailwind.config.js` 和 `postcss.config.js`。

- [ ] **Step 5: 配置 Tailwind**

文件 `frontend/tailwind.config.js`（替换全部）：

```js
/** @type {import('tailwindcss').Config} */
export default {
  darkMode: ["class"],
  content: [
    "./index.html",
    "./src/**/*.{ts,tsx}",
  ],
  theme: {
    container: {
      center: true,
      padding: "2rem",
      screens: { "2xl": "1400px" },
    },
    extend: {
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
    },
  },
  plugins: [],
};
```

- [ ] **Step 6: 写 index.css（Tailwind 指令 + shadcn 变量）**

文件 `frontend/src/index.css`（替换全部）：

```css
@tailwind base;
@tailwind components;
@tailwind utilities;

@layer base {
  :root {
    --background: 0 0% 100%;
    --foreground: 222.2 84% 4.9%;
    --card: 0 0% 100%;
    --card-foreground: 222.2 84% 4.9%;
    --popover: 0 0% 100%;
    --popover-foreground: 222.2 84% 4.9%;
    --primary: 222.2 47.4% 11.2%;
    --primary-foreground: 210 40% 98%;
    --secondary: 210 40% 96.1%;
    --secondary-foreground: 222.2 47.4% 11.2%;
    --muted: 210 40% 96.1%;
    --muted-foreground: 215.4 16.3% 46.9%;
    --accent: 210 40% 96.1%;
    --accent-foreground: 222.2 47.4% 11.2%;
    --destructive: 0 84.2% 60.2%;
    --destructive-foreground: 210 40% 98%;
    --border: 214.3 31.8% 91.4%;
    --input: 214.3 31.8% 91.4%;
    --ring: 222.2 84% 4.9%;
    --radius: 0.5rem;
  }

  .dark {
    --background: 222.2 84% 4.9%;
    --foreground: 210 40% 98%;
    --card: 222.2 84% 4.9%;
    --card-foreground: 210 40% 98%;
    --popover: 222.2 84% 4.9%;
    --popover-foreground: 210 40% 98%;
    --primary: 210 40% 98%;
    --primary-foreground: 222.2 47.4% 11.2%;
    --secondary: 217.2 32.6% 17.5%;
    --secondary-foreground: 210 40% 98%;
    --muted: 217.2 32.6% 17.5%;
    --muted-foreground: 215 20.2% 65.1%;
    --accent: 217.2 32.6% 17.5%;
    --accent-foreground: 210 40% 98%;
    --destructive: 0 62.8% 30.6%;
    --destructive-foreground: 210 40% 98%;
    --border: 217.2 32.6% 17.5%;
    --input: 217.2 32.6% 17.5%;
    --ring: 212.7 26.8% 83.9%;
  }
}

@layer base {
  * {
    @apply border-border;
  }
  body {
    @apply bg-background text-foreground;
  }
}
```

- [ ] **Step 7: 写 PostCSS 配置**

文件 `frontend/postcss.config.js`（替换为）：

```js
export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
};
```

- [ ] **Step 8: 配置 Vite（dev proxy）**

文件 `frontend/vite.config.ts`（替换全部）：

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  server: {
    port: 5173,
    proxy: {
      "/api": "http://127.0.0.1:8787",
      "/ws": {
        target: "ws://127.0.0.1:8787",
        ws: true,
      },
    },
  },
});
```

- [ ] **Step 9: 配置 tsconfig 支持 @ alias**

文件 `frontend/tsconfig.json`（替换全部）：

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["./src/*"]
    }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

文件 `frontend/tsconfig.node.json`（替换全部）：

```json
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true
  },
  "include": ["vite.config.ts"]
}
```

- [ ] **Step 10: 安装 shadcn/ui CLI 并初始化**

```powershell
cd frontend
npx shadcn@latest init
```

交互式选项：
- Style: Default
- Base color: Slate
- CSS variables: Yes

> 这会生成 `components.json` 和 `src/lib/utils.ts`（含 `cn()`）。若 `utils.ts` 未自动生成，手动创建。

若 `src/lib/utils.ts` 未生成，手动创建：

文件 `frontend/src/lib/utils.ts`：

```ts
import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

并安装依赖：

```powershell
cd frontend
npm install clsx tailwind-merge
```

- [ ] **Step 11: 添加 shadcn/ui 组件（card, button, badge）**

```powershell
cd frontend
npx shadcn@latest add card button badge
```

Expected: `src/components/ui/card.tsx`、`button.tsx`、`badge.tsx` 生成。

- [ ] **Step 12: 安装 TanStack Query 和 Zustand**

```powershell
cd frontend
npm install @tanstack/react-query zustand
```

- [ ] **Step 13: 添加 typecheck 脚本到 package.json**

修改 `frontend/package.json` 的 `scripts`：

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "preview": "vite preview",
    "typecheck": "tsc --noEmit"
  }
}
```

- [ ] **Step 14: 写最小 App.tsx 验证 Tailwind 生效**

文件 `frontend/src/App.tsx`（替换全部）：

```tsx
function App() {
  return (
    <div className="min-h-screen bg-background text-foreground">
      <div className="container">
        <h1 className="text-2xl font-bold mt-8">Vibe Dashboard</h1>
        <p className="text-muted-foreground mt-2">L1 基础设施层 - 脚手架就绪</p>
      </div>
    </div>
  );
}

export default App;
```

删除 Vite 模板自带的 `App.css`、`src/assets/` 等（如有）。

文件 `frontend/src/main.tsx`（替换为）：

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

- [ ] **Step 15: 验证 dev server 启动**

```powershell
cd frontend
npm run dev
```

Expected: 浏览器打开 http://localhost:5173 看到 "Vibe Dashboard" 标题，Tailwind 样式生效（深色文字、容器居中）。Ctrl+C 停止。

- [ ] **Step 16: 验证 typecheck 和 build**

```powershell
cd frontend
npm run typecheck
```

Expected: 无错误。

```powershell
cd frontend
npm run build
```

Expected: 构建成功，`dist/` 生成。

- [ ] **Step 17: Commit**

```powershell
cd E:\vibe-dashboard
git add frontend
git commit -m "chore(frontend): scaffold vite+react+ts+tailwind+shadcn"
```

---

## Task 9: 前端 API 客户端、WS 客户端与状态

**Files:**
- Create: `frontend/src/lib/api.ts`
- Create: `frontend/src/lib/ws.ts`
- Create: `frontend/src/lib/query.ts`
- Create: `frontend/src/stores/ui.ts`
- Create: `frontend/src/types/api.ts`

**Interfaces:**
- Consumes: 无
- Produces:
  - `lib/api.ts`：`request<T>(method, path, body?)`、`getJson<T>(path)`、`postJson<T>(path, body)`、`HealthResponse` 类型、`fetchHealth()`
  - `lib/ws.ts`：`WsClient` 类（自动重连、subscribe、send、连接状态）
  - `lib/query.ts`：`queryClient` 实例
  - `stores/ui.ts`：Zustand store 存 `wsStatus`、`connectionId`、`setWsStatus`、`pingPongLatency`

- [ ] **Step 1: 写类型定义**

文件 `frontend/src/types/api.ts`：

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
```

- [ ] **Step 2: 写 API 客户端**

文件 `frontend/src/lib/api.ts`：

```ts
export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
    this.name = "ApiError";
  }
}

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

  return res.json() as Promise<T>;
}

export function getJson<T>(path: string): Promise<T> {
  return request<T>("GET", path);
}

export function postJson<T>(path: string, body: unknown): Promise<T> {
  return request<T>("POST", path, body);
}

export function putJson<T>(path: string, body: unknown): Promise<T> {
  return request<T>("PUT", path, body);
}

export function del<T>(path: string): Promise<T> {
  return request<T>("DELETE", path);
}
```

- [ ] **Step 3: 写 WS 客户端**

文件 `frontend/src/lib/ws.ts`：

```ts
import type { ClientMsg, ServerMsg } from "@/types/api";

export type WsStatus = "connecting" | "open" | "closed";

type MsgHandler = (msg: ServerMsg) => void;
type StatusHandler = (status: WsStatus) => void;

const MAX_RETRIES = 5;
const BASE_DELAY_MS = 1000;

export class WsClient {
  private ws: WebSocket | null = null;
  private retries = 0;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;
  private msgHandlers = new Set<MsgHandler>();
  private statusHandlers = new Set<StatusHandler>();
  private status: WsStatus = "closed";

  connect(url: string) {
    this.cleanup();
    this.setStatus("connecting");
    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this.retries = 0;
      this.setStatus("open");
    };

    this.ws.onmessage = (event) => {
      try {
        const msg: ServerMsg = JSON.parse(event.data);
        this.msgHandlers.forEach((h) => h(msg));
      } catch (e) {
        console.error("invalid ws message", e);
      }
    };

    this.ws.onclose = () => {
      this.setStatus("closed");
      this.scheduleReconnect(url);
    };

    this.ws.onerror = (e) => {
      console.error("ws error", e);
    };
  }

  private scheduleReconnect(url: string) {
    if (this.retries >= MAX_RETRIES) {
      console.warn("ws max retries reached, stopping");
      return;
    }
    const delay = BASE_DELAY_MS * Math.pow(2, this.retries);
    this.retries += 1;
    this.retryTimer = setTimeout(() => this.connect(url), delay);
  }

  send(msg: ClientMsg) {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(msg));
    }
  }

  subscribe(handler: MsgHandler): () => void {
    this.msgHandlers.add(handler);
    return () => this.msgHandlers.delete(handler);
  }

  onStatus(handler: StatusHandler): () => void {
    this.statusHandlers.add(handler);
    handler(this.status);
    return () => this.statusHandlers.delete(handler);
  }

  private setStatus(status: WsStatus) {
    this.status = status;
    this.statusHandlers.forEach((h) => h(status));
  }

  private cleanup() {
    if (this.retryTimer) {
      clearTimeout(this.retryTimer);
      this.retryTimer = null;
    }
    if (this.ws) {
      this.ws.onopen = null;
      this.ws.onmessage = null;
      this.ws.onclose = null;
      this.ws.onerror = null;
      this.ws = null;
    }
  }

  disconnect() {
    this.retries = MAX_RETRIES;
    this.cleanup();
    this.setStatus("closed");
  }
}

export const wsClient = new WsClient();
```

- [ ] **Step 4: 写 Zustand store**

文件 `frontend/src/stores/ui.ts`：

```ts
import { create } from "zustand";
import type { WsStatus } from "@/lib/ws";

interface UiState {
  wsStatus: WsStatus;
  connectionId: string | null;
  pingPongLatency: number | null;
  setWsStatus: (status: WsStatus) => void;
  setConnectionId: (id: string | null) => void;
  setPingPongLatency: (ms: number) => void;
}

export const useUiStore = create<UiState>((set) => ({
  wsStatus: "closed",
  connectionId: null,
  pingPongLatency: null,
  setWsStatus: (wsStatus) => set({ wsStatus }),
  setConnectionId: (connectionId) => set({ connectionId }),
  setPingPongLatency: (pingPongLatency) => set({ pingPongLatency }),
}));
```

- [ ] **Step 5: 写 TanStack Query client**

文件 `frontend/src/lib/query.ts`：

```ts
import { QueryClient } from "@tanstack/react-query";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});
```

- [ ] **Step 6: 验证 typecheck**

```powershell
cd frontend
npm run typecheck
```

Expected: 无错误。

- [ ] **Step 7: Commit**

```powershell
cd E:\vibe-dashboard
git add frontend/src/lib frontend/src/stores frontend/src/types
git commit -m "feat(frontend): add api/ws clients and ui store"
```

---

## Task 10: 前端 HomePage 与端到端联调

**Files:**
- Create: `frontend/src/components/layout/Sidebar.tsx`
- Create: `frontend/src/pages/HomePage.tsx`
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/main.tsx`

**Interfaces:**
- Consumes: `lib/api`、`lib/ws`、`lib/query`、`stores/ui`、`types/api`、shadcn ui components
- Produces: 完整 L1 验收的前端页面（health 卡片 + WS 状态卡片 + ping 按钮 + 断线 banner）

- [ ] **Step 1: 写 Sidebar 占位组件**

文件 `frontend/src/components/layout/Sidebar.tsx`：

```tsx
import { Badge } from "@/components/ui/badge";

export function Sidebar() {
  return (
    <aside className="w-60 border-r bg-card min-h-screen p-4">
      <h2 className="text-lg font-semibold mb-4">Vibe Dashboard</h2>
      <nav className="space-y-2">
        <div className="flex items-center justify-between px-2 py-1.5 rounded-md hover:bg-accent cursor-pointer">
          <span className="text-sm">Workspaces</span>
          <Badge variant="secondary" className="text-xs">L2</Badge>
        </div>
      </nav>
      <div className="mt-8 text-xs text-muted-foreground px-2">
        基础设施层已就绪
      </div>
    </aside>
  );
}
```

- [ ] **Step 2: 写 HomePage**

文件 `frontend/src/pages/HomePage.tsx`：

```tsx
import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { getJson } from "@/lib/api";
import { wsClient } from "@/lib/ws";
import { useUiStore } from "@/stores/ui";
import type { HealthResponse, ServerMsg } from "@/types/api";

export function HomePage() {
  const { wsStatus, connectionId, pingPongLatency, setWsStatus, setConnectionId, setPingPongLatency } = useUiStore();
  const [pingSentAt, setPingSentAt] = useState<number | null>(null);

  const healthQuery = useQuery({
    queryKey: ["health"],
    queryFn: () => getJson<HealthResponse>("/api/health"),
    refetchInterval: 5000,
  });

  useEffect(() => {
    wsClient.connect(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`);

    const unsubStatus = wsClient.onStatus((status) => {
      setWsStatus(status);
      if (status === "open") {
        setPingSentAt(null);
        setPingPongLatency(0);
      }
    });

    const unsubMsg = wsClient.subscribe((msg: ServerMsg) => {
      if (msg.type === "hello") {
        setConnectionId(msg.payload.connection_id);
      } else if (msg.type === "pong") {
        if (pingSentAt) {
          setPingPongLatency(Date.now() - pingSentAt);
          setPingSentAt(null);
        }
      }
    });

    return () => {
      unsubStatus();
      unsubMsg();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handlePing = () => {
    setPingSentAt(Date.now());
    wsClient.send({ type: "ping" });
  };

  const wsStatusVariant = wsStatus === "open" ? "default" : wsStatus === "connecting" ? "secondary" : "destructive";

  return (
    <div className="flex min-h-screen">
      {/* sidebar placeholder moved to App.tsx layout */}
      <main className="flex-1 p-6">
        <h1 className="text-2xl font-bold mb-6">概览</h1>

        {wsStatus !== "open" && (
          <div className="mb-4 rounded-md border border-destructive bg-destructive/10 px-4 py-3 text-sm text-destructive">
            WebSocket 连接断开（{wsStatus}），正在尝试重连…
          </div>
        )}

        <div className="grid gap-4 md:grid-cols-2">
          <Card>
            <CardHeader>
              <CardTitle className="text-base">后端健康</CardTitle>
            </CardHeader>
            <CardContent className="space-y-2 text-sm">
              {healthQuery.isLoading && <p className="text-muted-foreground">加载中…</p>}
              {healthQuery.isError && (
                <p className="text-destructive">后端不可达：{(healthQuery.error as Error).message}</p>
              )}
              {healthQuery.data && (
                <>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">状态</span>
                    <Badge variant="default">{healthQuery.data.status}</Badge>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">版本</span>
                    <span className="font-mono">{healthQuery.data.version}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-muted-foreground">运行时长</span>
                    <span className="font-mono">{healthQuery.data.uptime_seconds.toFixed(1)}s</span>
                  </div>
                </>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle className="text-base">WebSocket 通道</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3 text-sm">
              <div className="flex justify-between items-center">
                <span className="text-muted-foreground">连接状态</span>
                <Badge variant={wsStatusVariant}>{wsStatus}</Badge>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">连接 ID</span>
                <span className="font-mono text-xs">
                  {connectionId ?? "—"}
                </span>
              </div>
              <div className="flex justify-between">
                <span className="text-muted-foreground">Ping 延迟</span>
                <span className="font-mono">
                  {pingPongLatency != null ? `${pingPongLatency}ms` : "—"}
                </span>
              </div>
              <Button onClick={handlePing} disabled={wsStatus !== "open"} size="sm">
                发送 Ping
              </Button>
            </CardContent>
          </Card>
        </div>
      </main>
    </div>
  );
}
```

- [ ] **Step 3: 写 App.tsx 布局 + Provider**

文件 `frontend/src/App.tsx`（替换全部）：

```tsx
import { QueryClientProvider } from "@tanstack/react-query";
import { queryClient } from "@/lib/query";
import { Sidebar } from "@/components/layout/Sidebar";
import { HomePage } from "@/pages/HomePage";

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <div className="flex min-h-screen">
        <Sidebar />
        <HomePage />
      </div>
    </QueryClientProvider>
  );
}

export default App;
```

- [ ] **Step 4: 确认 main.tsx 挂载 App**

文件 `frontend/src/main.tsx`（已在 Task 8 写好，确认无需改动）：

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

- [ ] **Step 5: typecheck**

```powershell
cd frontend
npm run typecheck
```

Expected: 无错误。

> 若报 `react-hooks/exhaustive-deps` 是 eslint 规则不是 tsc，typecheck 应通过。若装了 eslint 可单独跑。

- [ ] **Step 6: 端到端联调**

终端 A（后端）：

```powershell
cd backend
$env:VIBE_DB_PATH = "E:\vibe-dashboard\backend\dev.db"
cargo run -p api
```

Expected: 看到 `server starting addr=127.0.0.1:8787`。

终端 B（前端）：

```powershell
cd frontend
npm run dev
```

Expected: Vite 启动在 5173。

浏览器打开 http://localhost:5173：
- "后端健康"卡片显示 status=ok, version=0.1.0, uptime 递增
- "WebSocket 通道"卡片显示 connecting -> open，连接 ID 出现
- 点"发送 Ping"按钮，延迟数字出现（几十 ms）
- 终端 A Ctrl+C 停后端 -> 前端顶部出现红色"WebSocket 连接断开"banner，health 卡片显示"后端不可达"
- 重启后端 `cargo run -p api` -> 前端自动重连，banner 消失，health 恢复

- [ ] **Step 7: 最终质量门禁**

```powershell
cd backend
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

```powershell
cd frontend
npm run typecheck
npm run build
```

Expected: 全部通过。

- [ ] **Step 8: 生成 SQLx 离线缓存（为未来 CI 准备）**

```powershell
cd backend
$env:DATABASE_URL = "sqlite:./dev.db"
cargo sqlx prepare
```

Expected: 生成/更新 `backend/.sqlx/` 目录下 JSON 查询缓存文件。

> L1 的 `api` crate 没用 `query!` 宏，所以 `cargo sqlx prepare` 可能输出 "no queries found" 且不生成文件。这是预期行为。`db` crate 的 migration 不走 `query!` 宏（用 `sqlx::migrate!`），也不产生缓存。L2 才会有真正的 `query!` 用例。

- [ ] **Step 9: Commit**

```powershell
cd E:\vibe-dashboard
git add frontend/src backend/.sqlx
git commit -m "feat(frontend): add HomePage with health and ws status cards"
```

---

## Self-Review 结果

**1. Spec coverage**：
- ✅ Rust cargo workspace 多 crate（api/db/shared）-- Task 2
- ✅ Axum + tokio 后端 -- Task 7
- ✅ SQLite + WAL + foreign_keys -- Task 4
- ✅ SQLx migration 自动执行 -- Task 4
- ✅ `cargo sqlx prepare` 生成 `.sqlx/` -- Task 3 + Task 10 Step 8
- ✅ 配置走环境变量（VIBE_DB_PATH/VIBE_HTTP_PORT/VIBE_LOG_LEVEL）-- Task 5
- ✅ tracing JSON 日志 -- Task 3
- ✅ `GET /api/health` 返回 status/version/uptime -- Task 7
- ✅ `GET /ws` WebSocket 升级 -- Task 7
- ✅ WS Hub（Arc + DashMap）+ hello/ping/pong -- Task 6
- ✅ WS 心跳（30s ping / 10s 超时）-- **缺口**：spec 提到服务端每 30s 发 WebSocket ping 帧，10s 超时断开。当前 Task 6 的 session 没实现这个。但 axum 0.7 的 WebSocket 默认有心跳行为吗？实际上 axum 0.7 不自动发 ping。需要补。

> **修复**：在 Task 6 Step 3 的 `session.rs` 中加入服务端心跳任务。但因 axum 的 `Message::Ping` 需要客户端回 `Message::Pong`（不是我们的应用层 pong），且浏览器 WebSocket 自动回 ping 帧。所以服务端发 `Message::Ping`，浏览器自动回 `Message::Pong`，服务端检测超时即可。

**2. Placeholder scan**：无 TBD/TODO，所有步骤都有具体代码或命令。

**3. Type consistency**：
- `Hub::new() -> Arc<Hub>` 一致（Task 6 定义，Task 7 使用）
- `ConnId = Uuid` 一致
- `ClientMsg::Ping` / `ServerMsg::Hello` / `ServerMsg::Pong` 在前后端类型一致（Task 6 Rust + Task 9 TS）
- `HealthResponse { status, version, uptime_seconds }` 前后端字段名一致（Task 7 Rust + Task 9 TS）
- `AppState { db, hub, config, started_at }` 一致（Task 7）

**修复项**：补充 WS 心跳实现到 Task 6。

已在下方 Task 6 Step 3 的 `session.rs` 中加入心跳逻辑（已在最终版本中补充）。
