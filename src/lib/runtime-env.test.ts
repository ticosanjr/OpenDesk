import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(),
  openPath: vi.fn(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import {
  hideWindowIfTauri,
  isTauriRuntime,
  openExternalUrl,
  openLocalPath,
} from "./runtime-env";

function setTauri(enabled: boolean) {
  if (enabled) {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      value: {},
      configurable: true,
    });
  } else {
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
  }
}

afterEach(() => {
  setTauri(false);
  vi.restoreAllMocks();
  vi.clearAllMocks();
});

describe("runtime-env", () => {
  it("detects the browser by default and Tauri when the bridge exists", () => {
    expect(isTauriRuntime()).toBe(false);
    setTauri(true);
    expect(isTauriRuntime()).toBe(true);
  });

  it("opens http(s) URLs in a browser tab", async () => {
    const tab = vi.spyOn(window, "open").mockReturnValue({} as Window);
    await openExternalUrl("https://opencode.ai/product/browse");
    expect(tab).toHaveBeenCalledWith(
      "https://opencode.ai/product/browse",
      "_blank",
      "noopener,noreferrer",
    );
    expect(openUrl).not.toHaveBeenCalled();
  });

  it("rejects javascript and other non-http schemes", async () => {
    const tab = vi.spyOn(window, "open");
    await expect(openExternalUrl("javascript:alert(1)")).rejects.toThrow();
    await expect(openExternalUrl("file:///etc/passwd")).rejects.toThrow();
    expect(tab).not.toHaveBeenCalled();
    expect(openUrl).not.toHaveBeenCalled();
  });

  it("blocks invalid schemes even on desktop", async () => {
    setTauri(true);
    await expect(openExternalUrl("javascript:alert(1)")).rejects.toThrow();
    expect(openUrl).not.toHaveBeenCalled();
  });

  it("reports a popup blocked by the browser", async () => {
    vi.spyOn(window, "open").mockReturnValue(null);
    await expect(openExternalUrl("https://opencode.ai")).rejects.toThrow(
      /bloqueou/,
    );
  });

  it("delegates external URLs to the desktop opener", async () => {
    setTauri(true);
    vi.mocked(openUrl).mockResolvedValue(undefined);
    await openExternalUrl("https://opencode.ai");
    expect(openUrl).toHaveBeenCalledWith("https://opencode.ai/");
  });

  it("returns a fallback instead of throwing for local paths in PWA", async () => {
    expect(await openLocalPath("/tmp/data")).toEqual({
      ok: false,
      fallback: "/tmp/data",
    });
    expect(openPath).not.toHaveBeenCalled();
  });

  it("opens local paths on desktop and reports failures without throwing", async () => {
    setTauri(true);
    vi.mocked(openPath).mockResolvedValueOnce(undefined);
    expect(await openLocalPath("/tmp/data")).toEqual({ ok: true });

    vi.mocked(openPath).mockRejectedValueOnce(new Error("denied"));
    expect(await openLocalPath("/tmp/other")).toEqual({
      ok: false,
      fallback: "/tmp/other",
    });
  });

  it("does not try to hide a browser window", async () => {
    expect(await hideWindowIfTauri()).toBe(false);
  });
});
