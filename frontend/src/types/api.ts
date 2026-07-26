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
  | { type: "chat_thinking"; payload: { text: string; iteration: number } }
  | { type: "chat_tool_call"; payload: { tool_name: string; args: unknown } }
  | { type: "chat_tool_result"; payload: { tool_name: string; result: string } }
  | { type: "chat_error"; payload: { message: string } }
  | { type: "session_history"; payload: { messages: Array<{ role: string; content: string; tool_name: string | null }> } }
  // L5 review events
  | { type: "review_started"; payload: { review_id: string; job_id: string; todo_id: string } }
  | { type: "review_finding"; payload: { review_id: string; finding: ReviewFinding } }
  | { type: "review_completed"; payload: { review_id: string; summary: string; score: number; finding_count: number } }
  | { type: "review_error"; payload: { review_id: string; message: string } };

export type ClientMsg =
  | { type: "ping" }
  | { type: "chat_message"; payload: { text: string; workspace_id: string } }
  | { type: "new_session"; payload: { workspace_id: string } }
  | { type: "get_history"; payload: { workspace_id: string } };

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

// ---------- L5 审查层类型 ----------

export type ReviewStatus = "pending" | "in_progress" | "completed" | "failed";
export type FindingSeverity = "critical" | "major" | "minor" | "suggestion";

export interface Review {
  id: string;
  job_id: string;
  todo_id: string;
  status: ReviewStatus;
  summary: string;
  score: number | null;
  total_findings: number;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
}

export interface ReviewFinding {
  id: string;
  review_id: string;
  severity: FindingSeverity;
  file_path: string;
  line_number: number | null;
  category: string;
  title: string;
  description: string;
  suggestion: string;
  created_at: string;
}

export interface ReviewDetail {
  id: string;
  job_id: string;
  todo_id: string;
  status: ReviewStatus;
  summary: string;
  score: number | null;
  total_findings: number;
  created_at: string;
  updated_at: string;
  completed_at: string | null;
  findings: ReviewFinding[];
}

export interface CreateReviewInput {
  job_id: string;
  todo_id: string;
}

export interface CreateFindingInput {
  severity: FindingSeverity;
  file_path: string;
  line_number?: number | null;
  category: string;
  title: string;
  description: string;
  suggestion: string;
}

// ---------- L6 反馈闭环层类型 ----------

export interface ReviewFeedback {
  id: string;
  review_id: string;
  finding_id: string;
  todo_id: string | null;
  action: "pending" | "accepted" | "ignored" | "auto_fix";
  created_at: string;
  updated_at: string;
}

export type IterationStatus = "pending" | "running" | "passed" | "failed" | "maxed_out";

export interface ReviewIteration {
  id: string;
  todo_id: string;
  iteration: number;
  job_id: string | null;
  review_id: string | null;
  status: IterationStatus;
  score: number | null;
  threshold: number;
  summary: string;
  created_at: string;
  updated_at: string;
}
