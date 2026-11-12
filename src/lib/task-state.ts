export type TaskState =
  | "idle"
  | "queued"
  | "held"
  | "running"
  | "needs_approval"
  | "waiting_input"
  | "blocked"
  | "done"
  | "cancelled";

export interface TaskStateInput {
  status: string;
  phase_status?: string | null;
  held?: boolean;
  workflow_state?: string | null;
  workflow_reason?: string | null;
}

const RUNNING_PHASES = new Set(["queued", "running", "follow_up_queued"]);

export function isTerminalTaskStatus(status: string): boolean {
  return status === "DONE" || status === "BLOCKED" || status === "CANCELLED";
}

export function isTaskRunning(task: TaskStateInput): boolean {
  if (task.workflow_state) return task.workflow_state === "RUNNING";
  return task.status === "IN_PROGRESS" || RUNNING_PHASES.has(task.phase_status ?? "");
}

export function isTaskFailed(task: TaskStateInput): boolean {
  if (task.workflow_state) return task.workflow_reason === "failed";
  return Boolean(task.phase_status?.includes("failed"));
}

export function isTaskAwaitingApproval(task: TaskStateInput): boolean {
  if (task.workflow_state) return task.workflow_state === "NEEDS_APPROVAL";
  return task.phase_status === "awaiting_approval";
}

export function isTaskBlocked(task: TaskStateInput): boolean {
  return task.status === "BLOCKED" || isTaskFailed(task);
}

export function isTaskTerminal(task: TaskStateInput): boolean {
  return isTerminalTaskStatus(task.status);
}

export function taskState(task: TaskStateInput): TaskState {
  const workflow = task.workflow_state;
  if (workflow === "CANCELLED") return "cancelled";
  if (workflow === "DONE") return "done";
  if (workflow === "BLOCKED") return "blocked";
  if (workflow === "NEEDS_APPROVAL") return "needs_approval";
  if (workflow === "WAITING_INPUT") return "waiting_input";
  if (workflow === "RUNNING") return "running";
  if (workflow === "QUEUED") return "queued";
  if (workflow === "HELD") return "held";
  if (workflow === "READY") return "idle";
  if (!workflow && (task.phase_status === "queued" || task.phase_status === "follow_up_queued")) {
    return "queued";
  }
  if (task.status === "CANCELLED") return "cancelled";
  if (isTaskBlocked(task)) return "blocked";
  if (task.status === "DONE") return "done";
  if (isTaskAwaitingApproval(task)) return "needs_approval";
  if (isTaskRunning(task)) return "running";
  return "idle";
}
