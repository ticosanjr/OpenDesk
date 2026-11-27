import { afterEach, describe, expect, it, vi } from "vitest";
import { downloadBlob, downloadRun, downloadUrl, exportFilename } from "./download";

vi.mock("./api", () => ({
  apiBlob: vi.fn(async () => new Blob(["conteúdo"], { type: "text/markdown" })),
}));
const saveExportedData = vi.hoisted(() => vi.fn(async () => ({ saved: true })));
vi.mock("./import-export", () => ({ saveExportedData }));

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

  it("downloads a run through the shared export policy", async () => {
    await downloadRun("run-1", "json");
    expect(saveExportedData).toHaveBeenCalledWith(
      expect.objectContaining({
        filename: "opendesk-run-run-1.json",
        mime: "application/json",
      }),
    );
  });

  it("selects the MIME type for html and markdown exports", async () => {
    await downloadRun("run-html", "html");
    expect(saveExportedData).toHaveBeenLastCalledWith(
      expect.objectContaining({ mime: "text/html" }),
    );
    await downloadRun("run-md", "md");
    expect(saveExportedData).toHaveBeenLastCalledWith(
      expect.objectContaining({ mime: "text/markdown" }),
    );
  });

  it("downloads any authenticated URL through the shared export policy", async () => {
    const { apiBlob } = await import("./api");
    await downloadUrl("/api/files/raw?path=doc.md", "doc.md");
    expect(apiBlob).toHaveBeenCalledWith("/api/files/raw?path=doc.md");
    expect(saveExportedData).toHaveBeenCalledWith(
      expect.objectContaining({ filename: "doc.md" }),
    );
  });
});
