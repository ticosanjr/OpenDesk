import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  readKanbanViews,
  useKanbanViews,
  writeKanbanViews,
  type KanbanView,
} from "./kanban-views";

const base = {
  name: "Revisão",
  query: "",
  agent: "",
  priority: "",
  attentionOnly: true,
};

describe("useKanbanViews", () => {
  beforeEach(() => {
    localStorage.clear();
    vi.restoreAllMocks();
  });

  it("saves and removes a named view", () => {
    const { result } = renderHook(() => useKanbanViews());
    act(() => result.current.save(base));
    expect(result.current.views).toHaveLength(1);
    expect(result.current.views[0]?.name).toBe("Revisão");
    expect(readKanbanViews()).toHaveLength(1);

    const id = result.current.views[0]!.id;
    act(() => result.current.remove(id));
    expect(result.current.views).toHaveLength(0);
    expect(readKanbanViews()).toHaveLength(0);
  });

  it("keeps valid views and normalizes optional fields", () => {
    const stored: unknown = [
      { ...base, id: "a", query: "x" },
      { ...base, id: "b", attentionOnly: false },
      { ...base, id: "c", origin: "unknown", attentionOnly: "yes" },
      { id: "d" },
    ];
    writeKanbanViews(stored as KanbanView[]);
    const views = readKanbanViews();
    expect(views.map((view) => view.id)).toEqual(["a", "b", "c"]);
    expect(views[2]?.name).toBe("Revisão");
    expect(views[2]?.attentionOnly).toBe(false);
    expect(views[0]?.query).toBe("x");
  });

  it("falls back when storage is unavailable", () => {
    vi.stubGlobal("localStorage", undefined);
    const { result } = renderHook(() => useKanbanViews());
    act(() => result.current.save({ ...base, name: "Sem storage" }));
    expect(result.current.views).toHaveLength(1);
    expect(readKanbanViews()).toEqual([]);
    vi.unstubAllGlobals();
  });
});
