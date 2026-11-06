import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";

beforeEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    value: {},
    configurable: true,
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

describe("tauri endpoint resolution", () => {
  it("uses the invoke bridge when running inside Tauri", async () => {
    vi.mocked(invoke).mockResolvedValue({ port: 4321, token: "abc" });
    const fetchMock = vi
      .fn()
      .mockImplementation(async () => new Response("{}", { status: 200 }));
    vi.stubGlobal("fetch", fetchMock);
    const { apiGet } = await import("./api");

    await apiGet("/api/meta");
    const [url, init] = fetchMock.mock.calls[0];
    expect(invoke).toHaveBeenCalledWith("get_api_endpoint");
    expect(url).toBe("http://127.0.0.1:4321/api/meta");
    expect((init.headers as Headers).get("Authorization")).toBe("Bearer abc");
  });

  it("surfaces a missing endpoint in browser mode", async () => {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    vi.stubEnv("VITE_OPENDESK_PORT", "");
    vi.stubEnv("VITE_OPENDESK_TOKEN", "");
    const { getEndpoint } = await import("./api");
    await expect(getEndpoint()).rejects.toThrow("endpoint unavailable");
  });
});
