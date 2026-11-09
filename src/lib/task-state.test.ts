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
    expect(isTaskTerminal({ status: "DONE" })).toBe(true);
    expect(isTaskTerminal({ status: "IN_PROGRESS" })).toBe(false);
  });
});
