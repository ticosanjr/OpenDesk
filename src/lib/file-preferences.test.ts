import { describe, expect, it, vi, beforeEach } from "vitest";

const apiGet = vi.hoisted(() => vi.fn());
const apiPut = vi.hoisted(() => vi.fn());
vi.mock("./api", () => ({ apiGet, apiPut }));

import { loadFilePreferences, saveFilePreferences } from "./file-preferences";

describe("file preferences", () => {
  beforeEach(() => {
    apiGet.mockReset();
    apiPut.mockReset();
  });

  it("reads the configured values", async () => {
    apiGet.mockResolvedValue([
      { key: "download_dir", value_json: JSON.stringify("/tmp/downloads") },
      { key: "download_ask", value_json: "false" },
      { key: "html_allow_scripts", value_json: "true" },
    ]);
    await expect(loadFilePreferences()).resolves.toEqual({
      downloadDir: "/tmp/downloads",
      downloadAsk: false,
      htmlAllowScripts: true,
    });
  });

  it("falls back safely when settings are missing or broken", async () => {
    apiGet.mockResolvedValue([{ key: "download_ask", value_json: "not-json" }]);
    await expect(loadFilePreferences()).resolves.toEqual({
      downloadDir: "",
      downloadAsk: true,
      htmlAllowScripts: false,
    });
    apiGet.mockRejectedValue(new Error("offline"));
    await expect(loadFilePreferences()).resolves.toEqual({
      downloadDir: "",
      downloadAsk: true,
      htmlAllowScripts: false,
    });
  });

  it("persists the three preferences", async () => {
    apiPut.mockResolvedValue(undefined);
    await saveFilePreferences({
      downloadDir: " /tmp/downloads ",
      downloadAsk: false,
      htmlAllowScripts: true,
    });
    expect(apiPut).toHaveBeenCalledWith(
      "/api/settings/download_dir",
      "/tmp/downloads",
    );
    expect(apiPut).toHaveBeenCalledWith("/api/settings/download_ask", false);
    expect(apiPut).toHaveBeenCalledWith(
      "/api/settings/html_allow_scripts",
      true,
    );
  });
});
