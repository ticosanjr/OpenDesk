import { describe, expect, it } from "vitest";
import { matchesRunScope, runScope } from "./run-scope";
import type { TaskRun } from "@/lib/types";

function run(input: Partial<TaskRun>): TaskRun {
  return {
    id: "run",
    task_origin: null,
    task_id: "task",
    schedule_id: null,
    ...input,
  } as TaskRun;
}

describe("runScope", () => {
  it("classifies manual, routine, recipe and external automation runs", () => {
    expect(runScope(run({}))).toBe("work");
    expect(runScope(run({ task_origin: "CAPTURE" }))).toBe("work");
    expect(runScope(run({ task_origin: "ROUTINE" }))).toBe("routine");
    expect(runScope(run({ task_origin: "RECIPE" }))).toBe("recipe");
    expect(runScope(run({ task_origin: "IO_BUS" }))).toBe("automation");
    expect(runScope(run({ task_origin: "DELEGATION" }))).toBe("automation");
  });

  it("treats a schedule run without a task as a routine", () => {
    expect(runScope(run({ task_id: null, schedule_id: "s1" }))).toBe("routine");
  });

  it("filters the execution list", () => {
    const recipe = run({ task_origin: "RECIPE" });
    const routine = run({ task_origin: "ROUTINE" });
    const work = run({});
    expect(matchesRunScope(recipe, "")).toBe(true);
    expect(matchesRunScope(work, "work")).toBe(true);
    expect(matchesRunScope(recipe, "work")).toBe(false);
    expect(matchesRunScope(recipe, "automations")).toBe(true);
    expect(matchesRunScope(recipe, "recipe")).toBe(true);
    expect(matchesRunScope(routine, "recipe")).toBe(false);
  });
});
