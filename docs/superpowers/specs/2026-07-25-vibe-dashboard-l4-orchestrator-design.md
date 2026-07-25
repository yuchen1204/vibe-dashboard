# Vibe Dashboard - L4 编排层设计

**日期**: 2026-07-25
**状态**: 设计稿

## 背景与目标

L3 执行层已完成，用户可以点击 todo 上的"执行"按钮，由 ExecutorManager 调度 coding agent（Claude Code / OpenCode）在 git worktree 里执行任务。但当前流程是**手动触发**的：用户点一个 todo → 一个 agent 跑。

L4 **编排层**引入一个 LLM 驱动的"编排 Agent"作为大脑，让用户通过自然语言对话描述需求，编排 Agent 自动分解任务、调度多个 coding agent 并行/串行执行、汇总结果。

完成 L4 后，用户可以：在 workspace 内打开一个对话侧边栏 → 输入"给这个项目加一个 README" → 编排 Agent 自动创建 todo、分配 coding agent 执行、返回结果。

## 范围

**L4 做**：
- 新 crate `crates/orchestrator`：LLM 客户端 + 编排会话管理
- OpenAI 兼容 API 调用封装（支持 Anthropic / OpenAI / 本地 LLM）
- 编排 Agent 对话循环（system prompt + 上下文管理 + 工具调用）
- 工具：列出 target/todo、创建 todo、执行 todo、查看执行结果
- 聊天消息通过 WebSocket 双向传输
- 前端聊天侧边栏组件

**L4 不做**（明确推迟）：
- 多轮编排的持久化记忆（会话只保存在内存中，页面刷新重置）
- 审查层（L5 做）
- 流式 LLM 响应（L4 用非流式，简单稳定）
- 复杂的 Agent 循环（如自我反思、多步规划）
- 代码上下文自动注入（需要 RAG 或全量代码索引，留到 L5 或后续）

## 实体关系

```
OrchestratorSession (1) ──< (*) ChatMessage
     │
     │ 调用的工具
     ▼
- tasks::repo (创建/读取 todo)
- execution::executor (调度 coding agent)
- execution::repo (查询 job 状态)
```

## 数据模型

不新增 migration。聊天消息存在内存中（`Vec<ChatMessage>`），不落库。

```rust
pub struct ChatMessage {
    pub role: Role,        // User / Assistant / Tool
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub timestamp: DateTime<Utc>,
}

pub enum Role {
    User,
    Assistant,
    Tool,
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub result: Option<String>,
}
```

## Crate 结构

新增 `backend/crates/orchestrator`：

```
backend/crates/orchestrator/
├── Cargo.toml
└── src/
    ├── lib.rs          # re-export
    ├── llm.rs          # OpenAI 兼容 API 客户端
    ├── session.rs      # 会话管理（消息历史 + 上下文窗口）
    ├── tools.rs        # 可用工具定义 + 执行
    └── agent.rs        # 编排 Agent 循环
```

**依赖**：
- `orchestrator` 依赖：`shared`、`tasks`、`execution`、`reqwest`（HTTP 调用 LLM API）、`serde`、`serde_json`、`tokio`
- `orchestrator` dev-依赖：`tokio`（测试）
- `api` 新增依赖 `orchestrator`

## LLM 客户端设计

### 配置

```rust
pub struct LlmConfig {
    pub api_base: String,        // 默认 https://api.openai.com/v1
    pub api_key: String,         // 从环境变量读取
    pub model: String,           // 默认 claude-sonnet-5-20250725
    pub max_tokens: u32,         // 默认 4096
    pub temperature: f32,        // 默认 0.0
}
```

环境变量：`VIBE_LLM_API_BASE`、`VIBE_LLM_API_KEY`、`VIBE_LLM_MODEL`

### API 调用

实现 OpenAI 兼容的 `/v1/chat/completions` 调用，支持 tool_use（function calling）：

```rust
pub async fn chat_completion(
    messages: &[ChatMessage],
    tools: &[ToolDefinition],
    config: &LlmConfig,
) -> Result<LlmResponse>;
```

## 工具定义

编排 Agent 可以调用以下工具（function calling）：

| 工具名 | 参数 | 说明 |
|--------|------|------|
| `list_targets` | `workspace_id` | 列出 workspace 下的 target |
| `list_todos` | `workspace_id`, `target_id?` | 列出 todo（可按 target 过滤） |
| `create_todo` | `target_id`, `title`, `description?` | 创建新 todo |
| `execute_todo` | `todo_id` | 调度 coding agent 执行 todo |
| `get_job_result` | `job_id` | 查询执行结果 |

## 编排 Agent 循环

```
用户输入 → 追加到消息历史 → 调 LLM (含 tool definitions)
  → LLM 返回 text + tool_calls
  → 执行每个 tool_call → 结果追加到消息历史
  → 调 LLM (含 tool results)
  → LLM 返回最终回复 → 发给用户
```

## WebSocket 消息扩展

新增以下 WS 消息类型：

```jsonc
// 客户端 -> 服务端
{ "type": "chat_message", "payload": { "text": "帮我把这个项目加个 README" } }

// 服务端 -> 客户端
{ "type": "chat_response", "payload": { "text": "好的，我来..." } }
{ "type": "chat_tool_call", "payload": { "tool_name": "create_todo", "args": {...} } }
{ "type": "chat_tool_result", "payload": { "tool_name": "create_todo", "result": "..." } }
{ "type": "chat_error", "payload": { "message": "..." } }
```

## 前端设计

### 聊天侧边栏

新增组件 `frontend/src/components/chat/ChatSidebar.tsx`：

```
┌──────────────────────┐
│  AI 编排助手    [×]  │  ← 标题 + 关闭按钮
├──────────────────────┤
│                      │
│  用户: 加个 README   │  ← 消息气泡
│  助手: 好的，我...   │
│  🔧 创建 todo:      │  ← 工具调用显示
│     "编写 README"    │
│  ⏳ 执行中...        │
│  ✅ 完成! 见 ...     │
│                      │
├──────────────────────┤
│ [输入框...]    [发送] │  ← 输入区
└──────────────────────┘
```

### 交互流程

1. 用户打开 workspace 看板，右侧有聊天侧边栏（默认收起）
2. 输入文字 → 通过 WS 发送 `chat_message`
3. 后端编排 Agent 处理 → 通过 WS 返回 `chat_response` / `chat_tool_call` / `chat_tool_result`
4. 前端实时更新消息列表
5. 工具调用（如 `execute_todo`）触发 todo 列表自动刷新

## 测试策略

- `llm.rs`：mock HTTP 服务器测试 API 调用
- `session.rs`：消息历史管理、上下文窗口截断
- `tools.rs`：每个工具函数的单元测试
- `agent.rs`：mock LLM 响应测试编排循环
- 前端：手动验收

## 验收标准

- [ ] 环境变量 `VIBE_LLM_API_KEY` 配置后，发送 `chat_message` 能收到 LLM 回复
- [ ] 编排 Agent 能调用 `list_targets` / `list_todos` / `create_todo` / `execute_todo` 工具
- [ ] 工具执行结果能正确返回给 LLM 并生成最终回复
- [ ] 前端聊天侧边栏显示消息和工具调用
- [ ] `cargo test` 全过
- [ ] `cargo clippy --all-targets -- -D warnings` 无 warning
- [ ] `npm run typecheck` + `npm run build` 通过