import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

import { open } from "@tauri-apps/plugin-dialog";
import { joinPath, pickFile, pickFolder, slugifyWorkspace } from "./folder";

beforeEach(() => {
  vi.clearAllMocks();
});

describe("folder helpers", () => {
  it("slugifies names with accents and punctuation", () => {
    expect(slugifyWorkspace("Revisão Fiscal 2026")).toBe("revisao-fiscal-2026");
    expect(slugifyWorkspace("  Já foi!  ")).toBe("ja-foi");
    expect(slugifyWorkspace("___")).toMatch(/^workspace-/);
  });

  it("joins paths without duplicating separators", () => {
    expect(joinPath("/tmp/root", "child")).toBe("/tmp/root/child");
    expect(joinPath("/tmp/root/", "child")).toBe("/tmp/root/child");
    expect(joinPath("", "child")).toBe("child");
  });

  it("returns the selected folder or file", async () => {
    vi.mocked(open).mockResolvedValueOnce("/tmp/folder");
    await expect(pickFolder("Folder", "/tmp")).resolves.toBe("/tmp/folder");
    expect(open).toHaveBeenCalledWith(
      expect.objectContaining({ directory: true, multiple: false }),
    );

    vi.mocked(open).mockResolvedValueOnce("/tmp/opencode");
    await expect(pickFile("Binary")).resolves.toBe("/tmp/opencode");
    expect(open).toHaveBeenLastCalledWith(
      expect.objectContaining({ directory: false, multiple: false }),
    );
  });

  it("returns null on cancel or picker failure", async () => {
    vi.mocked(open).mockResolvedValueOnce(null);
    await expect(pickFolder("Folder")).resolves.toBeNull();

    vi.mocked(open).mockRejectedValueOnce(new Error("no picker"));
    await expect(pickFile("Binary")).resolves.toBeNull();
  });
});
