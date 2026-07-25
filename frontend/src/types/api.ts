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
  | { type: "pong"; payload: PongPayload }
  | { type: "job_output"; payload: { job_id: string; text: string } }
  | { type: "job_status"; payload: { job_id: string; todo_id: string; status: string } }
  | { type: "chat_response"; payload: { text: string } }
  | { type: "chat_tool_call"; payload: { tool_name: string; args: unknown } }
  | { type: "chat_tool_result"; payload: { tool_name: string; result: string } }
  | { type: "chat_error"; payload: { message: string } };

export type ClientMsg =
  | { type: "ping" }
  | { type: "chat_message"; payload: { text: string; workspace_id: string } };

// ---------- L3 执行层类型 ----------

export interface Worktree {
  id: string;
  workspace_id: string;
  target_id: string | null;
  branch: string;
  path: string;
  status: "active" | "merged" | "abandoned";
  created_at: string;
  updated_at: string;
}

export interface CreateWorktree {
  branch: string;
  target_id?: string;
}

export type JobStatus = "pending" | "running" | "success" | "failed" | "cancelled";

export interface ExecutionJob {
  id: string;
  todo_id: string;
  worktree_id: string | null;
  status: JobStatus;
  agent_type: string;
  prompt: string;
  output: string;
  started_at: string | null;
  finished_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface ExecuteTodo {
  agent_type?: string;
}

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
