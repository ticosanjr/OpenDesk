import { describe, expect, it } from "vitest";
import { links } from "./links";

describe("entity links", () => {
  it("encodes ids and paths", () => {
    expect(links.run("run 1")).toBe("/runs/run%201");
    expect(links.task("task/1")).toBe("/kanban?task=task%2F1");
    expect(links.artifact("art 1")).toBe("/artifacts?id=art%201");
    expect(links.schedule("sched 1")).toBe("/routines?schedule=sched%201");
    expect(links.file("output/a b.md")).toBe("/files?path=output%2Fa%20b.md");
  });

  it("builds agent and settings links", () => {
    expect(links.agent("build", "workspace", "ws 1")).toBe(
      "/studio?kind=agent&id=build&scope=workspace&workspace_id=ws+1",
    );
    expect(links.settings("engine")).toBe("/settings?section=engine");
    expect(links.settings("notifications")).toBe("/settings?section=notifications");
    expect(links.integration("google")).toBe("/integrations?section=google");
    expect(links.integration("mcp")).toBe("/integrations?section=mcp");
  });
});
