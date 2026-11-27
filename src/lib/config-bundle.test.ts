import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api", () => ({ apiPost: vi.fn() }));
const saveExportedData = vi.hoisted(() => vi.fn(async () => ({ saved: true })));
vi.mock("@/lib/import-export", () => ({ saveExportedData }));

import { apiPost } from "@/lib/api";
import { CONFIG_BUNDLE_PRESETS, downloadConfigBundle } from "./config-bundle";

beforeEach(() => {
  vi.clearAllMocks();
  (apiPost as ReturnType<typeof vi.fn>).mockResolvedValue({
    schema_version: 1,
    sections: {},
  });
});

describe("config bundle", () => {
  it("exposes the presets used by the UI", () => {
    expect(CONFIG_BUNDLE_PRESETS.all).toEqual([]);
    expect(CONFIG_BUNDLE_PRESETS.memory).toContain("structured_memory");
    expect(CONFIG_BUNDLE_PRESETS.prompts).toEqual(["prompt_templates"]);
    expect(CONFIG_BUNDLE_PRESETS.ops).toContain("governance");
  });

  it("downloads a JSON bundle for the selected sections", async () => {
    const name = await downloadConfigBundle("ws-1", ["prompt_templates"]);
    expect(apiPost).toHaveBeenCalledWith("/api/config-bundle/export", {
      workspace_id: "ws-1",
      sections: ["prompt_templates"],
    });
    expect(saveExportedData).toHaveBeenCalledTimes(1);
    const options = (saveExportedData as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(options.filename).toBe(name);
    expect(typeof options.content).toBe("string");
    expect(name).toMatch(/^opendesk-config-\d{4}-\d{2}-\d{2}\.json$/);
  });

  it("returns null when the export is cancelled", async () => {
    saveExportedData.mockResolvedValueOnce({ saved: false });
    await expect(downloadConfigBundle("ws-3")).resolves.toBeNull();
  });

  it("defaults to all sections", async () => {
    await downloadConfigBundle("ws-2");
    expect(apiPost).toHaveBeenCalledWith("/api/config-bundle/export", {
      workspace_id: "ws-2",
      sections: [],
    });
  });
});
