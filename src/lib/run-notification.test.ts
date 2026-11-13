import { describe, expect, it } from "vitest";
import { runNotification } from "./run-notification";

describe("runNotification", () => {
  it("names the task and reports duration, cost and tokens", () => {
    const value = runNotification("DONE", "Tarefa", {
      task_title: "Relatório mensal",
      agent_name: "build",
      duration_ms: 63_000,
      estimated_cost_usd: 0.042,
      tokens_input: 1200,
      tokens_output: 800,
    });
    expect(value.title).toBe("Tarefa concluída · Relatório mensal");
    expect(value.body).toContain("@build");
    expect(value.body).toContain("1m 03s");
    expect(value.body).toContain("0,042");
    expect(value.body).toContain("2.000 tokens");
  });

  it("includes the failure reason without leaking the whole log", () => {
    const value = runNotification("FAILED", "Tarefa", {
      task_title: "Diagnóstico",
      error_message: "timeout depois de 60s",
    });
    expect(value.title).toBe("Tarefa falhou · Diagnóstico");
    expect(value.body).toContain("timeout depois de 60s");
  });

  it("falls back when the run is unavailable", () => {
    expect(runNotification("CANCELLED", "Manual").title).toBe(
      "Tarefa cancelada · Manual",
    );
    expect(runNotification("DONE", "Manual").body).toContain("sucesso");
  });
});
