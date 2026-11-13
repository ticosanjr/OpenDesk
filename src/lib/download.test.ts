import { afterEach, describe, expect, it, vi } from "vitest";
import { downloadBlob, downloadRun, downloadUrl, exportFilename } from "./download";

vi.mock("./api", () => ({
  apiBlob: vi.fn(async () => new Blob(["conteúdo"], { type: "text/markdown" })),
}));

describe("export downloads", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("sanitizes run ids and keeps the format", () => {
    expect(exportFilename("run/1", "md")).toBe("opendesk-run-run-1.md");
    expect(exportFilename("run 1", "html")).toBe("opendesk-run-run-1.html");
    expect(exportFilename("", "json")).toBe("opendesk-run-run.json");
  });

  it("creates and revokes an object URL for the download", () => {
    vi.useFakeTimers();
    const create = vi.fn(() => "blob:export");
    const revoke = vi.fn();
    vi.stubGlobal("URL", { createObjectURL: create, revokeObjectURL: revoke });
    const click = vi
      .spyOn(HTMLAnchorElement.prototype, "click")
      .mockImplementation(() => {});
    downloadBlob(new Blob(["x"]), "run.md");
    expect(create).toHaveBeenCalled();
    expect(click).toHaveBeenCalled();
    vi.runAllTimers();
    expect(revoke).toHaveBeenCalledWith("blob:export");
    vi.useRealTimers();
  });

  it("downloads a run through the API blob", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("URL", {
      createObjectURL: () => "blob:run",
      revokeObjectURL: () => {},
    });
    const click = vi
      .spyOn(HTMLAnchorElement.prototype, "click")
      .mockImplementation(() => {});
    await downloadRun("run-1", "json");
    expect(click).toHaveBeenCalled();
    vi.runAllTimers();
    vi.useRealTimers();
  });

  it("downloads any authenticated URL through the API blob", async () => {
    const { apiBlob } = await import("./api");
    vi.useFakeTimers();
    vi.stubGlobal("URL", {
      createObjectURL: () => "blob:file",
      revokeObjectURL: () => {},
    });
    const click = vi
      .spyOn(HTMLAnchorElement.prototype, "click")
      .mockImplementation(() => {});

    await downloadUrl("/api/files/raw?path=doc.md", "doc.md");

    expect(apiBlob).toHaveBeenCalledWith("/api/files/raw?path=doc.md");
    expect(click).toHaveBeenCalled();
    vi.runAllTimers();
    vi.useRealTimers();
  });
});
