import { describe, expect, it } from "vitest";
import { buildInstallTask } from "./install-task";

describe("buildInstallTask", () => {
  it("carries every detail needed by the agent", () => {
    const task = buildInstallTask({
      kind: "skill",
      name: "Impeccable",
      source: "pbakaus/impeccable",
      sourceType: "github",
      url: "https://impeccable.style/#downloads",
      installCommand: "npx skills add pbakaus/impeccable",
      skillId: "impeccable",
      note: "Tem instalador próprio.",
      workspaceName: "Conteúdo",
    });
    expect(task.title).toBe("Instalar skill Impeccable");
    expect(task.description).toContain("pbakaus/impeccable");
    expect(task.description).toContain("https://impeccable.style/#downloads");
    expect(task.description).toContain("npx skills add");
    expect(task.description).toContain("Conteúdo");
    expect(task.tags).toEqual(["instalacao", "instalacao:skill"]);
  });

  it("still produces a usable task without optional fields", () => {
    const task = buildInstallTask({ kind: "mcp", name: "Memory" });
    expect(task.title).toBe("Instalar MCP Memory");
    expect(task.description).toContain("Critério de sucesso");
  });
});
