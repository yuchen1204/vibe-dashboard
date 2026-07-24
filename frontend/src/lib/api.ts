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