import type { TaskPermissionRequest } from "@/components/kanban/task-attention-modal";
import type { Task, TaskInteraction } from "@/lib/types";
import { applyRetention, type TaskVisibility } from "@/lib/task-visibility";
import { workflowMeta } from "@/lib/workflow";

export interface TaskAttentionEntry {
  interactions: TaskInteraction[];
  permissions: TaskPermissionRequest[];
}

export function buildAttentionByTask(attention: {
  task_interactions: TaskInteraction[];
  requests: TaskPermissionRequest[];
}): Map<string, TaskAttentionEntry> {
  const map = new Map<string, TaskAttentionEntry>();
  for (const interaction of attention.task_interactions) {
    const entry = map.get(interaction.task_id) ?? {
      interactions: [],
      permissions: [],
    };
    entry.interactions.push(interaction);
    map.set(interaction.task_id, entry);
  }
  for (const request of attention.requests) {
    if (!request.task_id) continue;
    const entry = map.get(request.task_id) ?? {
      interactions: [],
      permissions: [],
    };
    entry.permissions.push(request);
    map.set(request.task_id, entry);
  }
  return map;
}

export function filterKanbanTasks({
  tasks,
  query,
  agentFilter,
  priorityFilter,
  attentionOnly,
  attentionByTask,
  visibility,
}: {
  tasks: Task[];
  query: string;
  agentFilter: string;
  priorityFilter: string;
  attentionOnly: boolean;
  attentionByTask: Map<string, TaskAttentionEntry>;
  visibility: TaskVisibility;
}): Task[] {
  const needle = query.trim().toLowerCase();
  const filtered = tasks.filter((task) => {
    if (needle) {
      const haystack = `${task.title} ${task.description} ${task.tags_json} ${task.assigned_agent ?? ""}`.toLowerCase();
      if (!haystack.includes(needle)) return false;
    }
    if (agentFilter && (task.assigned_agent ?? "") !== agentFilter) return false;
    if (priorityFilter && String(task.priority) !== priorityFilter) return false;
    if (attentionOnly) {
      const pending = attentionByTask.get(task.id);
      const effective =
        pending && (pending.interactions.length > 0 || pending.permissions.length > 0)
          ? "WAITING_INPUT"
          : workflowMeta(task).state;
      if (!["NEEDS_APPROVAL", "WAITING_INPUT", "BLOCKED"].includes(effective)) {
        return false;
      }
    }
    return true;
  });
  return applyRetention(filtered, visibility)
    .slice()
    .sort((left, right) => right.created_at.localeCompare(left.created_at));
}

export function collectBoardAgents(tasks: Task[]): string[] {
  return Array.from(
    new Set(tasks.map((task) => task.assigned_agent).filter(Boolean)),
  ).sort() as string[];
}
