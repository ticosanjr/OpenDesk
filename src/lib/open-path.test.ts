import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
import { openWorkspacePath } from "./open-path";

describe("workspace paths", () => {
  it("delegates to the Rust allowlist command", async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    await openWorkspacePath("/tmp/workspace/file.txt");
    expect(invoke).toHaveBeenCalledWith("open_workspace_path", {
      path: "/tmp/workspace/file.txt",
    });
  });
});
