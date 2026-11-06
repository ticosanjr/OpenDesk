import { invoke } from "@tauri-apps/api/core";

export interface ApiEndpoint {
  port: number;
  token: string;
}

let endpointPromise: Promise<ApiEndpoint> | null = null;

export function getEndpoint(): Promise<ApiEndpoint> {
  if (!endpointPromise) {
    endpointPromise = resolveEndpoint().catch((error) => {
      endpointPromise = null;
      throw error;
    });
  }
  return endpointPromise;
}

async function resolveEndpoint(): Promise<ApiEndpoint> {
  if (typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
    return invoke<ApiEndpoint>("get_api_endpoint");
  }

  // Browser-only development fallback. Read the port/token from src-tauri's
  // api.json and export VITE_OPENDESK_PORT / VITE_OPENDESK_TOKEN.
  const token = import.meta.env.VITE_OPENDESK_TOKEN as string | undefined;
  const port = Number(import.meta.env.VITE_OPENDESK_PORT ?? 0);
  if (token && port > 0) {
    return { port, token };
  }

  throw new Error(
    "OpenDesk API endpoint unavailable. Run the app through `npm run tauri dev` or set VITE_OPENDESK_PORT and VITE_OPENDESK_TOKEN.",
  );
}

export async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  const { port, token } = await getEndpoint();
  const headers = new Headers(init.headers);
  headers.set("Authorization", `Bearer ${token}`);
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const response = await fetch(`http://127.0.0.1:${port}${path}`, {
    ...init,
    headers,
  });

  if (!response.ok) {
    let message = `Request failed (${response.status})`;
    try {
      const body = (await response.json()) as { message?: string };
      if (body.message) message = body.message;
    } catch {
      // Keep the status-based message.
    }
    throw new Error(message);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}

export async function apiGet<T>(path: string): Promise<T> {
  return apiFetch<T>(path);
}

export async function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return apiFetch<T>(path, {
    method: "POST",
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}

export async function apiPatch<T>(path: string, body?: unknown): Promise<T> {
  return apiFetch<T>(path, {
    method: "PATCH",
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}

export async function apiPut<T>(path: string, body?: unknown): Promise<T> {
  return apiFetch<T>(path, {
    method: "PUT",
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}

export async function apiDelete<T>(path: string): Promise<T> {
  return apiFetch<T>(path, { method: "DELETE" });
}

export async function websocketUrl(): Promise<string> {
  const { port, token } = await getEndpoint();
  return `ws://127.0.0.1:${port}/ws?token=${encodeURIComponent(token)}`;
}

export async function apiText(path: string): Promise<string> {
  const { port, token } = await getEndpoint();
  const response = await fetch(`http://127.0.0.1:${port}${path}`, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!response.ok) {
    throw new Error(`Request failed (${response.status})`);
  }
  return response.text();
}
