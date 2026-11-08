import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

beforeEach(() => {
  vi.resetModules();
  vi.stubEnv("VITE_OPENDESK_PORT", "1234");
  vi.stubEnv("VITE_OPENDESK_TOKEN", "token-123");
});

afterEach(() => {
  vi.unstubAllEnvs();
  vi.unstubAllGlobals();
});

describe("api client", () => {
  it("adds the bearer token and returns JSON", async () => {
    const fetchMock = vi.fn().mockResolvedValue(jsonResponse({ ok: true }));
    vi.stubGlobal("fetch", fetchMock);
    const { apiGet } = await import("./api");

    await expect(apiGet("/api/status")).resolves.toEqual({ ok: true });
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("http://127.0.0.1:1234/api/status");
    expect((init.headers as Headers).get("Authorization")).toBe("Bearer token-123");
  });

  it("sends JSON bodies for POST, PUT and PATCH", async () => {
    const fetchMock = vi
      .fn()
      .mockImplementation(async () => jsonResponse({ ok: true }));
    vi.stubGlobal("fetch", fetchMock);
    const { apiPatch, apiPost, apiPut } = await import("./api");

    await apiPost("/api/tasks", { title: "x" });
    await apiPut("/api/queue/concurrency", { concurrency: 2 });
    await apiPatch("/api/tasks/1", { priority: 3 });

    const postInit = fetchMock.mock.calls[0][1] as RequestInit;
    expect(postInit.method).toBe("POST");
    expect((postInit.headers as Headers).get("Content-Type")).toBe("application/json");
    expect(postInit.body).toBe(JSON.stringify({ title: "x" }));
    expect((fetchMock.mock.calls[1][1] as RequestInit).method).toBe("PUT");
    expect((fetchMock.mock.calls[2][1] as RequestInit).method).toBe("PATCH");
    expect((fetchMock.mock.calls[2][1] as RequestInit).body).toBe(
      JSON.stringify({ priority: 3 }),
    );
  });

  it("surfaces API error messages and handles 204", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(jsonResponse({ message: "OpenCode offline" }, 503))
      .mockResolvedValueOnce(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);
    const { apiGet, apiDelete } = await import("./api");

    await expect(apiGet("/api/engine/status")).rejects.toThrow("OpenCode offline");
    await expect(apiDelete("/api/x")).resolves.toBeUndefined();
  });

  it("builds the websocket URL with an encoded token", async () => {
    vi.stubEnv("VITE_OPENDESK_TOKEN", "a b&c");
    const { websocketUrl } = await import("./api");
    await expect(websocketUrl()).resolves.toBe(
      "ws://127.0.0.1:1234/ws?token=a%20b%26c",
    );
  });
});

describe("apiText", () => {
  it("returns raw text and rejects failures", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(new Response("log line", { status: 200 }))
      .mockResolvedValueOnce(new Response("nope", { status: 500 }));
    vi.stubGlobal("fetch", fetchMock);
    const { apiText } = await import("./api");

    await expect(apiText("/api/runs/1/log")).resolves.toBe("log line");
    await expect(apiText("/api/missing")).rejects.toThrow("Request failed (500)");
  });
});
