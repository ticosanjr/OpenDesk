export type TaskState =
  | "idle"
  | "running"
  | "needs_approval"
  | "blocked"
  | "done"
  | "cancelled";

export interface TaskStateInput {
  status: string;
  phase_status?: string | null;
}

const RUNNING_PHASES = new Set(["queued", "running", "follow_up_queued"]);

export function isTerminalTaskStatus(status: string): boolean {
  return status === "DONE" || status === "BLOCKED" || status === "CANCELLED";
}

export function isTaskRunning(task: TaskStateInput): boolean {
  return task.status === "IN_PROGRESS" || RUNNING_PHASES.has(task.phase_status ?? "");
}

export function isTaskFailed(task: TaskStateInput): boolean {
  return Boolean(task.phase_status?.includes("failed"));
}

export function isTaskAwaitingApproval(task: TaskStateInput): boolean {
  return task.phase_status === "awaiting_approval";
}

export function isTaskBlocked(task: TaskStateInput): boolean {
  return task.status === "BLOCKED" || isTaskFailed(task);
}

export function isTaskTerminal(task: TaskStateInput): boolean {
  return isTerminalTaskStatus(task.status);
}

export function taskState(task: TaskStateInput): TaskState {
  if (task.status === "CANCELLED") return "cancelled";
  if (isTaskBlocked(task)) return "blocked";
  if (task.status === "DONE") return "done";
  if (isTaskAwaitingApproval(task)) return "needs_approval";
  if (isTaskRunning(task)) return "running";
  return "idle";
}
