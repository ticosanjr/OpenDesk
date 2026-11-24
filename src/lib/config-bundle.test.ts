import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/api", () => ({ apiPost: vi.fn() }));
vi.mock("@/lib/download", () => ({ downloadBlob: vi.fn() }));

import { apiPost } from "@/lib/api";
import { downloadBlob } from "@/lib/download";
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
    expect(downloadBlob).toHaveBeenCalledTimes(1);
    const [blob, filename] = (downloadBlob as ReturnType<typeof vi.fn>).mock
      .calls[0];
    expect(blob).toBeInstanceOf(Blob);
    expect(filename).toBe(name);
    expect(name).toMatch(/^opendesk-config-\d{4}-\d{2}-\d{2}\.json$/);
  });

  it("defaults to all sections", async () => {
    await downloadConfigBundle("ws-2");
    expect(apiPost).toHaveBeenCalledWith("/api/config-bundle/export", {
      workspace_id: "ws-2",
      sections: [],
    });
  });
});
