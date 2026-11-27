import { beforeEach, describe, expect, it, vi } from "vitest";

const save = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/plugin-dialog", () => ({ save }));
const apiPost = vi.hoisted(() => vi.fn());
const apiBlob = vi.hoisted(() => vi.fn());
vi.mock("./api", () => ({ apiPost, apiBlob }));
const downloadBlob = vi.hoisted(() => vi.fn());
vi.mock("./download", () => ({ downloadBlob }));
const loadFilePreferences = vi.hoisted(() => vi.fn());
vi.mock("./file-preferences", () => ({ loadFilePreferences }));

import { saveLocalFile } from "./save-file";

describe("saveLocalFile", () => {
  beforeEach(() => {
    save.mockReset();
    apiPost.mockReset();
    apiBlob.mockReset();
    loadFilePreferences.mockResolvedValue({
      downloadDir: "/tmp/downloads",
      downloadAsk: true,
      importExportAsk: true,
      htmlAllowScripts: false,
    });
  });

  it("asks for a destination and saves a workspace file", async () => {
    save.mockResolvedValue("/home/user/relatorio.md");
    apiPost.mockResolvedValue({});
    await expect(
      saveLocalFile({
        filename: "relatorio.md",
        workspaceId: "ws-1",
        path: "output/relatorio.md",
      }),
    ).resolves.toBe(true);
    expect(save).toHaveBeenCalledWith({
      title: "Salvar arquivo",
      defaultPath: "/tmp/downloads/relatorio.md",
    });
    expect(apiPost).toHaveBeenCalledWith("/api/files/save", {
      workspace_id: "ws-1",
      path: "output/relatorio.md",
      destination: "/home/user/relatorio.md",
    });
  });

  it("saves an artifact directly when asking is disabled", async () => {
    loadFilePreferences.mockResolvedValue({
      downloadDir: "/tmp/downloads",
      downloadAsk: false,
      importExportAsk: true,
      htmlAllowScripts: false,
    });
    apiPost.mockResolvedValue({});
    await saveLocalFile({ filename: "a.zip", artifactId: "art-1" });
    expect(save).not.toHaveBeenCalled();
    expect(apiPost).toHaveBeenCalledWith("/api/artifacts/art-1/save", {
      destination: "/tmp/downloads/a.zip",
    });
  });

  it("returns false when the dialog is cancelled", async () => {
    save.mockResolvedValue(null);
    await expect(saveLocalFile({ filename: "a.md" })).resolves.toBe(false);
    expect(apiPost).not.toHaveBeenCalled();
  });

  it("returns false when the dialog is unavailable and there is no fallback", async () => {
    save.mockRejectedValue(new Error("no tauri"));
    await expect(saveLocalFile({ filename: "a.md" })).resolves.toBe(false);
  });

  it("falls back to a browser download when the dialog is unavailable", async () => {
    save.mockRejectedValue(new Error("no tauri"));
    apiBlob.mockResolvedValue(new Blob(["x"]));
    await expect(
      saveLocalFile({
        filename: "a.md",
        fallbackUrl: "/api/files/raw?path=a.md",
      }),
    ).resolves.toBe(true);
    expect(downloadBlob).toHaveBeenCalled();
  });
});
