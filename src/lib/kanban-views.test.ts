import { beforeEach, describe, expect, it, vi } from "vitest";
import { readKanbanViews, writeKanbanViews } from "./kanban-views";

const view = {
  id: "v1",
  name: "Revisão",
  origin: "all" as const,
  query: "",
  agent: "",
  priority: "",
  attentionOnly: true,
};

describe("kanban views", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("persists and restores views", () => {
    writeKanbanViews([view]);
    expect(readKanbanViews()).toEqual([view]);
  });

  it("ignores malformed entries", () => {
    localStorage.setItem(
      "opendesk.kanban.views.v1",
      JSON.stringify([view, { name: 1 }, "x"]),
    );
    expect(readKanbanViews()).toEqual([view]);
  });

  it("survives unavailable storage", () => {
    vi.stubGlobal("localStorage", undefined);
    expect(() => writeKanbanViews([view])).not.toThrow();
    expect(readKanbanViews()).toEqual([]);
    vi.unstubAllGlobals();
  });
});
