import type { Dispatch, SetStateAction } from "react";
import { apiPatch, apiPost, apiPut } from "@/lib/api";
import type { DispatchDecision, Task } from "@/lib/types";

export interface KanbanActionsContext {
  load: () => Promise<void>;
  setError: Dispatch<SetStateAction<string | null>>;
  setTasks: Dispatch<SetStateAction<Task[]>>;
  setDecision: Dispatch<SetStateAction<DispatchDecision | null>>;
  setSelectedTask: Dispatch<SetStateAction<Task | null>>;
}

export function createKanbanActions({
  load,
  setError,
  setTasks,
  setDecision,
  setSelectedTask,
}: KanbanActionsContext) {
  async function approve(task: Task) {
    setError(null);
    try {
      const response = await apiPost<{ decision: DispatchDecision }>(
        `/api/tasks/${task.id}/approve-plan`,
        {},
      );
      setDecision(response.decision);
      setSelectedTask(null);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function transitionTask(task: Task, status: string, reason?: string) {
    if (!status || status === task.status) return;
    setError(null);
    if (status !== "IN_PROGRESS") {
      setTasks((current) =>
        current.map((item) => (item.id === task.id ? { ...item, status } : item)),
      );
    }
    try {
      await apiPost(`/api/tasks/${task.id}/transition`, {
        status,
        reason: reason ?? null,
      });
      // Moving a card is not an outcome: no toast/native notification.
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      await load();
    }
  }

  async function holdTask(task: Task, held: boolean) {
    setError(null);
    try {
      await apiPost(`/api/tasks/${task.id}/hold`, { held });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      await load();
    }
  }

  async function setWaiting(task: Task) {
    await holdTask(task, true);
  }

  async function toggleQueue(pause: boolean) {
    setError(null);
    try {
      await apiPost(
        pause
          ? "/api/queue/pause?reason=paused%20from%20Kanban"
          : "/api/queue/resume",
      );
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function setConcurrency(value: number) {
    setError(null);
    try {
      await apiPut("/api/queue/concurrency", { concurrency: value });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function updateJobPriority(id: string, priority: number) {
    try {
      await apiPatch(`/api/queue/${id}/priority`, { priority });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  async function jobAction(id: string, action: "cancel" | "retry") {
    try {
      await apiPost(`/api/queue/${id}/${action}`);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  return {
    approve,
    transitionTask,
    holdTask,
    setWaiting,
    toggleQueue,
    setConcurrency,
    updateJobPriority,
    jobAction,
  };
}
