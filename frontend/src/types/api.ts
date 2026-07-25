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
