# Vibe Dashboard L4 编排层 Implementation Plan

**Goal:** 在 L3 执行层上构建编排层：LLM 客户端封装、编排 Agent 会话循环（function calling 工具调度）、WS 聊天通道、前端聊天侧边栏。

**Spec:** `docs/superpowers/specs/2026-07-25-vibe-dashboard-l4-orchestrator-design.md`

## Global Constraints

- 沿用 L1-L3 全部约束。
- 新 crate `orchestrator` 依赖 `shared`、`tasks`、`execution`，加 `reqwest` 做 HTTP 客户端。
- 每个 task 结束前必须 `cargo fmt --all` + `cargo clippy --all-targets -- -D warnings` + `cargo test` 通过。
- 平台：Windows（PowerShell）。

---

## Task 1: orchestrator crate 骨架 + LLM 客户端

**Files:**
- Modify: `backend/Cargo.toml`（members 加 orchestrator）
- Create: `backend/crates/orchestrator/Cargo.toml`
- Create: `backend/crates/orchestrator/src/lib.rs`
- Create: `backend/crates/orchestrator/src/llm.rs`
- Create: `backend/crates/orchestrator/src/session.rs`
- Create: `backend/crates/orchestrator/src/tools.rs`
- Create: `backend/crates/orchestrator/src/agent.rs`

## Task 2: 会话管理 + 工具定义

## Task 3: 编排 Agent 循环

## Task 4: WS 消息扩展 + API 路由

## Task 5: 前端聊天侧边栏

## Task 6: 端到端联调 + 质量门禁