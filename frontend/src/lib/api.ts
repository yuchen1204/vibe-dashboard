import type { Review, ReviewDetail, CreateReviewInput, CreateFindingInput, ReviewFinding } from "@/types/api";

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

  if (res.status === 204) {
    return undefined as T;
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

export interface PathSuggestResponse {
  paths: string[];
}

export async function getPathSuggestions(q: string): Promise<string[]> {
  const data = await getJson<PathSuggestResponse>(
    `/api/path-suggest?q=${encodeURIComponent(q)}`,
  );
  return data.paths;
}

// ---------- LLM 配置 ----------

export interface LlmConfigResponse {
  api_base: string;
  model: string;
  configured: boolean;
}

export interface LlmConfigInput {
  api_base?: string;
  api_key?: string;
  model?: string;
}

export function getLlmConfig(): Promise<LlmConfigResponse> {
  return getJson<LlmConfigResponse>("/api/settings/llm");
}

export function saveLlmConfig(input: LlmConfigInput): Promise<void> {
  return putJson("/api/settings/llm", input);
}

export function clearLlmConfig(): Promise<void> {
  return del("/api/settings/llm");
}

// ---------- Review API ----------

export function listReviewsByTodo(todoId: string): Promise<Review[]> {
  return getJson<Review[]>(`/api/reviews/todo/${todoId}`);
}

export function listReviewsByJob(jobId: string): Promise<Review[]> {
  return getJson<Review[]>(`/api/reviews/job/${jobId}`);
}

export function getReviewDetail(id: string): Promise<ReviewDetail> {
  return getJson<ReviewDetail>(`/api/reviews/${id}`);
}

export function createReview(input: CreateReviewInput): Promise<Review> {
  return postJson<Review>("/api/reviews", input);
}

export function addFinding(
  reviewId: string,
  input: CreateFindingInput,
): Promise<ReviewFinding> {
  return postJson<ReviewFinding>(`/api/reviews/${reviewId}/findings`, input);
}

export function updateReviewSummary(
  reviewId: string,
  input: { summary?: string; score?: number; total_findings?: number },
): Promise<Review> {
  return putJson<Review>(`/api/reviews/${reviewId}/summary`, input);
}
