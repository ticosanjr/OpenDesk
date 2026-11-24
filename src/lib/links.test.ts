import { describe, expect, it } from "vitest";
import { links } from "./links";

describe("navigation links", () => {
  it("sends queue and board deep links to the right Kanban view", () => {
    expect(links.dashboard()).toBe("/");
    expect(links.kanban()).toBe("/kanban?view=board");
    expect(links.kanban("task/1")).toBe("/kanban?view=board&task=task%2F1");
    expect(links.queue()).toBe("/kanban?view=queue");
    expect(links.queue("job 1")).toBe("/kanban?view=queue&job=job%201");
    expect(links.task("task 1")).toBe("/kanban?view=board&task=task%201");
    expect(links.taskRun("task 1", "run/1")).toBe(
      "/kanban?view=board&task=task%201&run=run%2F1",
    );
  });

  it("keeps run, chat and calendar links canonical", () => {
    expect(links.run("run 1")).toBe("/runs/run%201");
    expect(links.runs()).toBe("/runs");
    expect(links.runs("active")).toBe("/runs?scope=active");
    expect(links.chat()).toBe("/chat");
    expect(links.chat("thread 1")).toBe("/chat?thread=thread%201");
    expect(links.calendar()).toBe("/calendar");
    expect(links.calendar("2026-11-12")).toBe("/calendar?date=2026-11-12");
    expect(links.calendarEvent("evt 1")).toBe("/calendar?event=evt%201");
  });

  it("encodes artifacts, routines and recipes", () => {
    expect(links.artifact("art 1")).toBe("/artifacts?artifact=art%201");
    expect(links.routine("sched 1")).toBe("/routines?routine=sched%201");
    expect(links.schedule("sched 1")).toBe("/routines?routine=sched%201");
    expect(links.recipe("recipe/1")).toBe("/recipes?recipe=recipe%2F1");
    expect(links.recipeRun("run 1")).toBe("/recipe-runs/run%201");
  });

  it("supports studio tabs with and without focus", () => {
    expect(links.studio("skills")).toBe("/studio?tab=skills");
    expect(links.studio("agents", "build/1")).toBe(
      "/studio?tab=agents&focus=build%2F1",
    );
  });
});

describe("settings and integration links", () => {
  it("uses default sections and encodes the requested section", () => {
    expect(links.settings()).toBe("/settings?section=general");
    expect(links.settings("engine")).toBe("/settings?section=engine");
    expect(links.settings("sandbox")).toBe("/settings?section=sandbox");
    expect(links.settings("voice")).toBe("/settings?section=voice");
    expect(links.integration()).toBe("/integrations?section=smtp");
    expect(links.integration("google")).toBe("/integrations?section=google");
    expect(links.integration("team")).toBe("/integrations?section=team");
    expect(links.integration("vault")).toBe("/integrations?section=vault");
  });

  it("builds learning and governance links", () => {
    expect(links.learning()).toBe("/learning");
    expect(links.learning("proposals")).toBe("/learning?tab=proposals");
    expect(links.governance()).toBe("/governance");
    expect(links.governance("goals")).toBe("/governance?section=goals");
  });
});

describe("entity links", () => {
  it("builds agent and skill links", () => {
    expect(links.agent("build")).toContain("kind=agent");
    expect(links.agent("build")).not.toContain("workspace_id");
    expect(links.agent("build", "workspace", "ws 1")).toBe(
      "/studio?kind=agent&id=build&scope=workspace&workspace_id=ws+1",
    );
    expect(links.skill("learned-failure-1", "workspace", "ws 1")).toBe(
      "/studio?kind=skill&id=learned-failure-1&scope=workspace&workspace_id=ws+1",
    );
  });

  it("encodes files and directories", () => {
    expect(links.file("output/a b.md")).toBe("/files?path=output%2Fa%20b.md");
    expect(links.dir("output/a b")).toBe("/files?dir=1&path=output%2Fa%20b");
  });
});
