import { describe, expect, it } from "vitest";
import {
  applyRetention,
  DEFAULT_VISIBILITY,
  loadVisibility,
  saveVisibility,
} from "./task-visibility";
import type { Task } from "@/lib/types";

function task(overrides: Partial<Task> & { id: string }): Task {
  return {
    workspace_id: "ws-1",
    title: overrides.id,
    description: "",
    status: "DONE",
    phase: "build",
    phase_status: null,
    priority: 2,
    execution_mode: "AUTO_DSH",
    assigned_agent: null,
    preferred_model: null,
    agent_override: false,
    model_override: false,
    effort_level: "medium",
    plan_markdown: null,
    output_dir_path: null,
    notify_email: null,
    export_to_gdocs: false,
    tags_json: "[]",
    position_index: 0,
    last_run_id: null,
    last_run_status: null,
    last_run_at: null,
    blocked_reason: null,
    cancelled_at: null,
    forked_from_task_id: null,
    created_at: "2026-01-01T10:00:00Z",
    updated_at: "2026-01-01T10:00:00Z",
    ...overrides,
  } as Task;
}

describe("task visibility", () => {
  it("never hides active tasks", () => {
    const now = new Date("2026-11-07T12:00:00Z");
    const tasks = [
      task({ id: "old-todo", status: "TODO", updated_at: "2020-01-01T00:00:00Z" }),
      task({ id: "old-done", status: "DONE", updated_at: "2020-01-01T00:00:00Z" }),
    ];
    const result = applyRetention(tasks, { retention: "1h", maxItems: 0 }, now);
    expect(result.map((item) => item.id)).toEqual(["old-todo"]);
  });

  it("keeps only the newest N finished cards", () => {
    const now = new Date("2026-11-07T12:00:00Z");
    const tasks = Array.from({ length: 5 }).map((_, index) =>
      task({
        id: `done-${index}`,
        status: "DONE",
        updated_at: `2026-11-07T0${index}:00:00Z`,
      }),
    );
    const result = applyRetention(tasks, { retention: "all", maxItems: 2 }, now);
    expect(result.map((item) => item.id).sort()).toEqual(["done-3", "done-4"]);
  });

  it("persists the per-workspace preference", () => {
    localStorage.clear();
    expect(loadVisibility("ws-1")).toEqual(DEFAULT_VISIBILITY);
    saveVisibility("ws-1", { retention: "today", maxItems: 3 });
    expect(loadVisibility("ws-1")).toEqual({ retention: "today", maxItems: 3 });
    expect(loadVisibility("ws-2")).toEqual(DEFAULT_VISIBILITY);
  });
});

describe("task visibility edge cases", () => {
  it("filters today and seven-day windows", () => {
    const now = new Date("2026-11-08T12:00:00Z");
    const tasks = [
      task({ id: "today", status: "DONE", updated_at: "2026-11-08T09:00:00Z" }),
      task({ id: "yesterday", status: "DONE", updated_at: "2026-11-07T09:00:00Z" }),
      task({ id: "last-week", status: "CANCELLED", updated_at: "2026-11-02T09:00:00Z" }),
    ];
    expect(
      applyRetention(tasks, { retention: "today", maxItems: 0 }, now).map(
        (item) => item.id,
      ),
    ).toEqual(["today"]);
    expect(
      applyRetention(tasks, { retention: "7d", maxItems: 0 }, now).map(
        (item) => item.id,
      ),
    ).toEqual(["today", "yesterday", "last-week"]);
  });

  it("sanitizes malformed storage and ignores missing workspaces", () => {
    localStorage.clear();
    localStorage.setItem("opendesk.kanban.visibility.ws-1", "not-json");
    expect(loadVisibility("ws-1")).toEqual(DEFAULT_VISIBILITY);
    localStorage.setItem(
      "opendesk.kanban.visibility.ws-1",
      JSON.stringify({ retention: "never", maxItems: -4 }),
    );
    expect(loadVisibility("ws-1")).toEqual(DEFAULT_VISIBILITY);
    expect(() => saveVisibility(null, DEFAULT_VISIBILITY)).not.toThrow();
  });
});
