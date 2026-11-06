import { beforeEach, describe, expect, it, vi } from "vitest";

const apiGet = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api", () => ({
  apiGet,
}));

import {
  invalidateAgents,
  invalidateModels,
  invalidateRuntime,
  loadAgents,
  loadModelHealth,
  loadModels,
  loadRuntime,
} from "./studio-cache";

function agent(id: string, scope = "workspace") {
  return {
    id,
    scope,
    path: `/tmp/${id}.md`,
    hidden: false,
    disabled: false,
    permissions: [],
    system: "",
    sha256: id,
  };
}

describe("studio cache", () => {
  beforeEach(() => {
    apiGet.mockReset();
    invalidateAgents();
    invalidateModels();
    invalidateRuntime();
  });

  it("merges global and workspace agents and caches the result", async () => {
    apiGet.mockImplementation((path: string) => {
      if (path.includes("scope=workspace")) return Promise.resolve([agent("local")]);
      return Promise.resolve([agent("global", "global"), agent("local")]);
    });

    const first = await loadAgents("ws-1");
    expect(first.map((item) => item.id)).toEqual(["global", "local"]);
    expect(apiGet).toHaveBeenCalledTimes(2);

    const second = await loadAgents("ws-1");
    expect(second).toBe(first);
    expect(apiGet).toHaveBeenCalledTimes(2);
  });

  it("loads global agents without a workspace and survives failures", async () => {
    apiGet.mockRejectedValue(new Error("offline"));
    const list = await loadAgents(null);
    expect(list).toEqual([]);
  });

  it("caches models and health", async () => {
    apiGet.mockImplementation((path: string) => {
      if (path.includes("health")) return Promise.resolve({ models: [{ code: "x" }] });
      return Promise.resolve({ data: [{ id: "m" }] });
    });

    const models = await loadModels();
    const health = await loadModelHealth();
    expect(models).toHaveLength(1);
    expect(health).toHaveLength(1);
    await loadModels();
    await loadModelHealth();
    expect(apiGet).toHaveBeenCalledTimes(2);
  });

  it("returns empty models when the API fails", async () => {
    apiGet.mockRejectedValue(new Error("offline"));
    await expect(loadModels()).resolves.toEqual([]);
    await expect(loadModelHealth()).resolves.toEqual([]);
  });

  it("loads and caches runtime inventory", async () => {
    apiGet.mockResolvedValue({ engine: {}, agents: [], models: [], skills: [], commands: [], mcps: [] });
    const first = await loadRuntime("ws-1");
    const second = await loadRuntime("ws-1");
    expect(first).toBe(second);
    expect(apiGet).toHaveBeenCalledTimes(1);
  });

  it("returns null runtime without a workspace or on failure", async () => {
    expect(await loadRuntime(null)).toBeNull();
    apiGet.mockRejectedValue(new Error("offline"));
    expect(await loadRuntime("ws-1")).toBeNull();
  });
});
