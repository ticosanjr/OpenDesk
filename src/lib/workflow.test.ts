import { describe, expect, it } from "vitest";
import { fallbackWorkflow, workflowMeta } from "./workflow";

describe("workflow mapping", () => {
  it("uses the backend workflow state and reason", () => {
    const meta = workflowMeta({
      status: "IN_PROGRESS",
      workflow_state: "RUNNING",
      workflow_reason: null,
    });
    expect(meta.state).toBe("RUNNING");
    expect(meta.tone).toBe("blue");
    expect(meta.label).toBe("executando");
  });

  it("explains approval, HITL and permission reasons", () => {
    expect(
      workflowMeta({
        status: "IN_REVIEW",
        workflow_state: "NEEDS_APPROVAL",
        workflow_reason: "approval",
      }).reason,
    ).toBe("aprovação de plano");
    expect(
      workflowMeta({
        status: "IN_REVIEW",
        workflow_state: "WAITING_INPUT",
        workflow_reason: "hitl",
      }).reason,
    ).toBe("pergunta do agente");
    expect(
      workflowMeta({
        status: "IN_REVIEW",
        workflow_state: "WAITING_INPUT",
        workflow_reason: "permission",
      }).reason,
    ).toBe("permissão necessária");
  });

  it("does not treat a manual In progress move as running", () => {
    const fallback = fallbackWorkflow({
      status: "IN_PROGRESS",
      phase_status: null,
    });
    expect(fallback).toEqual({
      state: "BLOCKED",
      reason: "in_progress_without_run",
    });
    expect(
      workflowMeta({
        status: "IN_PROGRESS",
        workflow_state: "BLOCKED",
        workflow_reason: "in_progress_without_run",
      }).reason,
    ).toBe("sem execução ativa");
  });

  it("falls back to queue and ready states", () => {
    expect(fallbackWorkflow({ status: "TODO", phase_status: "queued" })).toEqual({
      state: "QUEUED",
      reason: "queued",
    });
    expect(fallbackWorkflow({ status: "TODO", phase_status: "forked" })).toEqual({
      state: "READY",
      reason: "ready",
    });
  });
});

describe("workflow metadata coverage", () => {
  it("labels every backend state", () => {
    const cases: Array<[string, string, string]> = [
      ["READY", "pronta", "default"],
      ["QUEUED", "na fila", "yellow"],
      ["HELD", "em espera", "default"],
      ["RUNNING", "executando", "blue"],
      ["NEEDS_APPROVAL", "aprovar plano", "yellow"],
      ["WAITING_INPUT", "aguardando você", "yellow"],
      ["BLOCKED", "atenção", "red"],
      ["DONE", "concluída", "green"],
      ["CANCELLED", "cancelada", "default"],
    ];
    for (const [state, label, tone] of cases) {
      const meta = workflowMeta({
        status: "TODO",
        workflow_state: state,
      });
      expect(meta.label).toBe(label);
      expect(meta.tone).toBe(tone);
    }
  });

  it("explains every workflow reason", () => {
    const reasons = [
      ["ready", "pronta para executar"],
      ["queued", "na fila"],
      ["held", "em espera"],
      ["approval", "aprovação de plano"],
      ["hitl", "pergunta do agente"],
      ["permission", "permissão necessária"],
      ["failed", "falhou"],
      ["manual_block", "bloqueada manualmente"],
      ["in_progress_without_run", "sem execução ativa"],
      ["done", "concluída"],
      ["cancelled", "cancelada"],
    ] as const;
    for (const [reason, label] of reasons) {
      expect(
        workflowMeta({
          status: "TODO",
          workflow_state: "READY",
          workflow_reason: reason,
        }).reason,
      ).toBe(label);
    }
  });

  it("falls back for unknown states and reasons", () => {
    const meta = workflowMeta({
      status: "TODO",
      workflow_state: "FUTURE_STATE",
      workflow_reason: "unknown_reason",
    });
    expect(meta.label).toBe("pronta");
    expect(meta.reason).toBe("unknown_reason");
  });

  it("maps every legacy fallback branch", () => {
    expect(fallbackWorkflow({ status: "CANCELLED" })).toEqual({
      state: "CANCELLED",
      reason: "cancelled",
    });
    expect(fallbackWorkflow({ status: "DONE" })).toEqual({
      state: "DONE",
      reason: "done",
    });
    expect(
      fallbackWorkflow({ status: "IN_PROGRESS", phase_status: "failed" }),
    ).toEqual({ state: "BLOCKED", reason: "failed" });
    expect(fallbackWorkflow({ status: "BLOCKED" })).toEqual({
      state: "BLOCKED",
      reason: "manual_block",
    });
    expect(
      fallbackWorkflow({ status: "IN_REVIEW", phase_status: "awaiting_approval" }),
    ).toEqual({ state: "NEEDS_APPROVAL", reason: "approval" });
    expect(
      fallbackWorkflow({
        status: "IN_REVIEW",
        phase_status: "awaiting_permission",
      }),
    ).toEqual({ state: "WAITING_INPUT", reason: "permission" });
    expect(
      fallbackWorkflow({ status: "IN_REVIEW", phase_status: "awaiting_input" }),
    ).toEqual({ state: "WAITING_INPUT", reason: "hitl" });
    expect(
      fallbackWorkflow({ status: "IN_REVIEW", phase_status: "waiting_hitl" }),
    ).toEqual({ state: "WAITING_INPUT", reason: "hitl" });
    expect(
      fallbackWorkflow({ status: "IN_PROGRESS", phase_status: "running" }),
    ).toEqual({ state: "RUNNING" });
    expect(fallbackWorkflow({ status: "IN_REVIEW" })).toEqual({
      state: "WAITING_INPUT",
      reason: "hitl",
    });
    expect(fallbackWorkflow({ status: "TODO", phase_status: "held" })).toEqual({
      state: "HELD",
      reason: "held",
    });
    expect(
      fallbackWorkflow({ status: "TODO", phase_status: "follow_up_queued" }),
    ).toEqual({ state: "QUEUED", reason: "queued" });
    expect(
      fallbackWorkflow({ status: "TODO", held: true }),
    ).toEqual({ state: "HELD", reason: "held" });
  });
});
