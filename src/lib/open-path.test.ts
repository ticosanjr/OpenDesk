import { afterEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { openWorkspacePath } from "./open-path";

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

afterEach(() => setTauri(false));

describe("workspace paths", () => {
  it("delegates to the Rust allowlist command on desktop", async () => {
    setTauri(true);
    vi.mocked(invoke).mockResolvedValue(undefined);
    await openWorkspacePath("/tmp/workspace/file.txt");
    expect(invoke).toHaveBeenCalledWith("open_workspace_path", {
      path: "/tmp/workspace/file.txt",
    });
  });

  it("fails closed outside the desktop runtime", async () => {
    setTauri(false);
    await expect(openWorkspacePath("/tmp/workspace/file.txt")).rejects.toThrow(
      /desktop/i,
    );
    expect(invoke).not.toHaveBeenCalled();
  });
});
