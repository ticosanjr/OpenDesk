import type { Task } from "@/lib/types";

export type RetentionWindow = "1h" | "today" | "7d" | "all";

export interface TaskVisibility {
  /** How long finished/cancelled cards stay visible on the board. */
  retention: RetentionWindow;
  /** Hard cap for finished/cancelled cards; 0 means unlimited. */
  maxItems: number;
}

export const DEFAULT_VISIBILITY: TaskVisibility = {
  retention: "7d",
  maxItems: 20,
};

export const RETENTION_OPTIONS: Array<{
  value: RetentionWindow;
  label: string;
  hint: string;
}> = [
  { value: "1h", label: "Última hora", hint: "Só o que terminou na última hora" },
  { value: "today", label: "Hoje", hint: "Desde a meia-noite local" },
  { value: "7d", label: "Últimos 7 dias", hint: "Padrão recomendado" },
  { value: "all", label: "Tudo", hint: "Nunca ocultar concluídas" },
];

export function visibilityKey(workspaceId: string): string {
  return `opendesk.kanban.visibility.${workspaceId}`;
}

export function loadVisibility(workspaceId?: string | null): TaskVisibility {
  if (!workspaceId) return DEFAULT_VISIBILITY;
  try {
    const raw = localStorage.getItem(visibilityKey(workspaceId));
    if (!raw) return DEFAULT_VISIBILITY;
    const parsed = JSON.parse(raw) as Partial<TaskVisibility>;
    return {
      retention:
        parsed.retention &&
        RETENTION_OPTIONS.some((option) => option.value === parsed.retention)
          ? parsed.retention
          : DEFAULT_VISIBILITY.retention,
      maxItems:
        typeof parsed.maxItems === "number" && parsed.maxItems >= 0
          ? Math.floor(parsed.maxItems)
          : DEFAULT_VISIBILITY.maxItems,
    };
  } catch {
    return DEFAULT_VISIBILITY;
  }
}

export function saveVisibility(
  workspaceId: string | null | undefined,
  value: TaskVisibility,
): void {
  if (!workspaceId) return;
  try {
    localStorage.setItem(visibilityKey(workspaceId), JSON.stringify(value));
  } catch {
    // Storage can be unavailable in private modes; the UI still works for the
    // current session with the in-memory value.
  }
}

function retentionCutoff(window: RetentionWindow, now: Date): number | null {
  switch (window) {
    case "1h":
      return now.getTime() - 60 * 60 * 1000;
    case "today": {
      const start = new Date(now);
      start.setHours(0, 0, 0, 0);
      return start.getTime();
    }
    case "7d":
      return now.getTime() - 7 * 24 * 60 * 60 * 1000;
    default:
      return null;
  }
}

/**
 * Hides old DONE/CANCELLED cards only. Active work is never filtered, and the
 * data itself is untouched — this is a purely visual retention policy.
 */
export function applyRetention(
  tasks: Task[],
  visibility: TaskVisibility,
  now: Date = new Date(),
): Task[] {
  const cutoff = retentionCutoff(visibility.retention, now);
  const finishedIds = new Set<string>();
  const visibleFinished: Task[] = [];

  const result = tasks.filter((task) => {
    if (task.status !== "DONE" && task.status !== "CANCELLED") return true;
    finishedIds.add(task.id);
    const stamp = task.last_run_at ?? task.updated_at ?? task.created_at;
    if (cutoff !== null) {
      const time = Date.parse(stamp);
      if (Number.isFinite(time) && time < cutoff) return false;
    }
    visibleFinished.push(task);
    return true;
  });

  if (visibility.maxItems <= 0) return result;

  const ordered = visibleFinished
    .slice()
    .sort(
      (left, right) =>
        Date.parse(right.last_run_at ?? right.updated_at ?? right.created_at) -
        Date.parse(left.last_run_at ?? left.updated_at ?? left.created_at),
    );
  const keep = new Set(ordered.slice(0, visibility.maxItems).map((task) => task.id));

  return result.filter(
    (task) => !finishedIds.has(task.id) || keep.has(task.id),
  );
}
