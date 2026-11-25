import { invoke } from "@tauri-apps/api/core";

export interface ApiEndpoint {
  host?: string;
  port: number;
  token: string;
  baseUrl?: string;
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
  const envToken = import.meta.env.VITE_OPENDESK_TOKEN as string | undefined;
  const envPort = Number(import.meta.env.VITE_OPENDESK_PORT ?? 0);
  if (envToken && envPort > 0) {
    return { port: envPort, token: envToken };
  }

  // PWA served by opendeskd: same-origin API and a token stored after login.
  const stored = window.localStorage.getItem("opendesk_api_token");
  if (stored) {
    return {
      port: Number(window.location.port || 80),
      token: stored,
      baseUrl: window.location.origin,
    };
  }

  throw new Error(
    "OpenDesk API endpoint unavailable. Run the app through `npm run tauri dev`, set VITE_OPENDESK_PORT and VITE_OPENDESK_TOKEN, or sign in to the PWA.",
  );
}

function apiBase(endpoint: ApiEndpoint): string {
  return endpoint.baseUrl ?? `http://127.0.0.1:${endpoint.port}`;
}

export class ApiError extends Error {
  status: number;
  retryAfter: number | null;

  constructor(message: string, status: number, retryAfter: number | null = null) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.retryAfter = retryAfter;
  }
}

function retryAfterSeconds(response: Response): number | null {
  const raw = response.headers.get("retry-after");
  if (!raw) return null;
  const value = Number(raw);
  return Number.isFinite(value) && value >= 0 ? value : null;
}

export async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
  const endpoint = await getEndpoint();
  const headers = new Headers(init.headers);
  headers.set("Authorization", `Bearer ${endpoint.token}`);
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  const response = await fetch(`${apiBase(endpoint)}${path}`, {
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
    throw new ApiError(message, response.status, retryAfterSeconds(response));
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return (await response.json()) as T;
}

export async function apiGet<T>(path: string): Promise<T> {
  try {
    return await apiFetch<T>(path);
  } catch (error) {
    if (!(error instanceof ApiError) || error.status !== 429) throw error;
    const delay = Math.min(2000, Math.max(250, (error.retryAfter ?? 1) * 1000));
    await new Promise((resolve) => window.setTimeout(resolve, delay));
    return apiFetch<T>(path);
  }
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
  const endpoint = await getEndpoint();
  const base = apiBase(endpoint);
  const url = new URL(base);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  url.pathname = "/ws";
  url.search = `?token=${encodeURIComponent(endpoint.token)}`;
  return url.toString();
}

export async function apiPostBlob(path: string, body?: unknown): Promise<Blob> {
  const endpoint = await getEndpoint();
  const response = await fetch(`${apiBase(endpoint)}${path}`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${endpoint.token}`,
      "Content-Type": "application/json",
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) {
    let message = `Request failed (${response.status})`;
    try {
      const parsed = (await response.json()) as { message?: string };
      if (parsed.message) message = parsed.message;
    } catch {
      // Keep the status-based message.
    }
    throw new Error(message);
  }
  return response.blob();
}

export async function apiBlob(path: string): Promise<Blob> {
  const endpoint = await getEndpoint();
  const response = await fetch(`${apiBase(endpoint)}${path}`, {
    headers: { Authorization: `Bearer ${endpoint.token}` },
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
  return response.blob();
}

export async function apiText(path: string): Promise<string> {
  const endpoint = await getEndpoint();
  const response = await fetch(`${apiBase(endpoint)}${path}`, {
    headers: { Authorization: `Bearer ${endpoint.token}` },
  });
  if (!response.ok) {
    throw new Error(`Request failed (${response.status})`);
  }
  return response.text();
}
