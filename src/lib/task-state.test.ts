import { describe, expect, it } from "vitest";
import {
  isTaskAwaitingApproval,
  isTaskBlocked,
  isTaskFailed,
  isTaskRunning,
  isTaskTerminal,
  taskState,
} from "./task-state";

describe("task state", () => {
  it("derives running from status or active phase", () => {
    expect(isTaskRunning({ status: "IN_PROGRESS" })).toBe(true);
    expect(isTaskRunning({ status: "TODO", phase_status: "running" })).toBe(true);
    expect(isTaskRunning({ status: "TODO", phase_status: "follow_up_queued" })).toBe(true);
    expect(isTaskRunning({ status: "TODO", phase_status: "plan_complete" })).toBe(false);
    expect(
      isTaskRunning({
        status: "IN_PROGRESS",
        workflow_state: "BLOCKED",
        workflow_reason: "in_progress_without_run",
      }),
    ).toBe(false);
    expect(isTaskRunning({ status: "IN_PROGRESS", workflow_state: "RUNNING" })).toBe(true);
  });

  it("separates approval, failure and block", () => {
    expect(isTaskAwaitingApproval({ status: "IN_REVIEW", phase_status: "awaiting_approval" })).toBe(true);
    expect(isTaskFailed({ status: "IN_PROGRESS", phase_status: "failed" })).toBe(true);
    expect(isTaskBlocked({ status: "BLOCKED" })).toBe(true);
    expect(isTaskBlocked({ status: "IN_PROGRESS", phase_status: "failed" })).toBe(true);
    expect(isTaskBlocked({ status: "IN_PROGRESS", phase_status: "running" })).toBe(false);
  });

  it("projects a single state and terminal status", () => {
    expect(taskState({ status: "CANCELLED" })).toBe("cancelled");
    expect(taskState({ status: "BLOCKED", phase_status: "failed" })).toBe("blocked");
    expect(taskState({ status: "DONE" })).toBe("done");
    expect(taskState({ status: "IN_REVIEW", phase_status: "awaiting_approval" })).toBe("needs_approval");
    expect(taskState({ status: "IN_PROGRESS" })).toBe("running");
    expect(taskState({ status: "TODO" })).toBe("idle");
    expect(taskState({ status: "TODO", workflow_state: "QUEUED" })).toBe("queued");
    expect(taskState({ status: "TODO", workflow_state: "HELD" })).toBe("held");
    expect(taskState({ status: "IN_REVIEW", workflow_state: "WAITING_INPUT" })).toBe(
      "waiting_input",
    );
    expect(
      taskState({
        status: "IN_PROGRESS",
        workflow_state: "BLOCKED",
        workflow_reason: "in_progress_without_run",
      }),
    ).toBe("blocked");
    expect(isTaskTerminal({ status: "DONE" })).toBe(true);
    expect(isTaskTerminal({ status: "IN_PROGRESS" })).toBe(false);
  });
});

describe("task workflow coverage", () => {
  it("maps every backend workflow state", () => {
    const cases: Array<[string, string]> = [
      ["CANCELLED", "cancelled"],
      ["DONE", "done"],
      ["BLOCKED", "blocked"],
      ["NEEDS_APPROVAL", "needs_approval"],
      ["WAITING_INPUT", "waiting_input"],
      ["RUNNING", "running"],
      ["QUEUED", "queued"],
      ["HELD", "held"],
      ["READY", "idle"],
    ];
    for (const [workflow, expected] of cases) {
      expect(taskState({ status: "TODO", workflow_state: workflow })).toBe(
        expected,
      );
    }
  });

  it("honors workflow flags over legacy status", () => {
    expect(
      isTaskFailed({
        status: "IN_PROGRESS",
        workflow_state: "BLOCKED",
        workflow_reason: "failed",
      }),
    ).toBe(true);
    expect(
      isTaskFailed({
        status: "IN_PROGRESS",
        workflow_state: "BLOCKED",
        workflow_reason: "manual_block",
      }),
    ).toBe(false);
    expect(
      isTaskAwaitingApproval({
        status: "IN_REVIEW",
        workflow_state: "NEEDS_APPROVAL",
      }),
    ).toBe(true);
    expect(
      isTaskAwaitingApproval({
        status: "IN_REVIEW",
        workflow_state: "WAITING_INPUT",
      }),
    ).toBe(false);
  });

  it("keeps the legacy fallback branch for all states", () => {
    expect(taskState({ status: "BLOCKED" })).toBe("blocked");
    expect(taskState({ status: "DONE" })).toBe("done");
    expect(taskState({ status: "IN_REVIEW", phase_status: "awaiting_approval" })).toBe(
      "needs_approval",
    );
    expect(taskState({ status: "IN_REVIEW", phase_status: "awaiting_input" })).toBe(
      "idle",
    );
    expect(taskState({ status: "IN_PROGRESS", phase_status: "running" })).toBe(
      "running",
    );
    expect(taskState({ status: "TODO", phase_status: "queued" })).toBe("queued");
    expect(taskState({ status: "TODO", phase_status: "follow_up_queued" })).toBe(
      "queued",
    );
  });
});
