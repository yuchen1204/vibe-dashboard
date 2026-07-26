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
