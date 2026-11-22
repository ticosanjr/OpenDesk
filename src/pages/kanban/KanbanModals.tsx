import type { Dispatch, SetStateAction } from "react";
import {
  CreateTaskModal,
  EditTaskModal,
  ForkTaskModal,
  ExecutionModal,
  LogDrawer,
  SwarmModal,
  TaskDetailsModal,
} from "@/components/kanban/task-modals";
import { apiPost } from "@/lib/api";
import type { Task } from "@/lib/types";

export function KanbanModals({
  createOpen,
  setCreateOpen,
  workspaceId,
  load,
  selectedTask,
  setSelectedTask,
  editTask,
  setEditTask,
  executeTask,
  setExecuteTask,
  forkSource,
  setForkSource,
  swarmTask,
  setSwarmTask,
  logTask,
  setLogTask,
  initialRunId,
  setInitialRunId,
  lastRunEvent,
  setNotice,
  setError,
  approve,
  transitionTask,
  setWaiting,
}: {
  createOpen: boolean;
  setCreateOpen: Dispatch<SetStateAction<boolean>>;
  workspaceId: string;
  load: () => Promise<void>;
  selectedTask: Task | null;
  setSelectedTask: Dispatch<SetStateAction<Task | null>>;
  editTask: Task | null;
  setEditTask: Dispatch<SetStateAction<Task | null>>;
  executeTask: Task | null;
  setExecuteTask: Dispatch<SetStateAction<Task | null>>;
  forkSource: Task | null;
  setForkSource: Dispatch<SetStateAction<Task | null>>;
  swarmTask: Task | null;
  setSwarmTask: Dispatch<SetStateAction<Task | null>>;
  logTask: Task | null;
  setLogTask: Dispatch<SetStateAction<Task | null>>;
  initialRunId: string | null;
  setInitialRunId: Dispatch<SetStateAction<string | null>>;
  lastRunEvent: { run_id: string; status: string } | null;
  setNotice: Dispatch<SetStateAction<string | null>>;
  setError: Dispatch<SetStateAction<string | null>>;
  approve: (task: Task) => void;
  transitionTask: (task: Task, status: string) => void;
  setWaiting: (task: Task) => void;
}) {
  return (
    <>
      <CreateTaskModal
        open={createOpen}
        workspaceId={workspaceId}
        onClose={() => setCreateOpen(false)}
        onCreated={async () => {
          setCreateOpen(false);
          await load();
        }}
      />

      <TaskDetailsModal
        task={selectedTask}
        onClose={() => setSelectedTask(null)}
        onEdit={(task) => {
          setEditTask(task);
          setSelectedTask(null);
        }}
        onApprove={(task) => void approve(task)}
        onDispatch={(task) => {
          setExecuteTask(task);
          setSelectedTask(null);
        }}
        onStatus={(task, status) => void transitionTask(task, status)}
        onWait={(task) => void setWaiting(task)}
        onFork={(task) => {
          setSelectedTask(null);
          setForkSource(task);
        }}
        onSwarm={(task) => {
          setSwarmTask(task);
          setSelectedTask(null);
        }}
        onLogs={(task) => {
          setLogTask(task);
          setInitialRunId(null);
          setSelectedTask(null);
        }}
      />

      <ExecutionModal
        open={!!executeTask}
        task={executeTask}
        onClose={() => setExecuteTask(null)}
        onChanged={async () => {
          await load();
        }}
      />

      <ForkTaskModal
        open={!!forkSource}
        source={forkSource}
        onClose={() => setForkSource(null)}
        onSaved={async () => {
          setForkSource(null);
          await load();
        }}
      />

      <EditTaskModal
        task={editTask}
        onClose={() => setEditTask(null)}
        onSaved={async (task, rerun) => {
          setEditTask(null);
          setNotice(
            rerun
              ? `Tarefa salva e reenfileirada: ${task.title}`
              : `Tarefa salva: ${task.title}`,
          );
          if (rerun) {
            try {
              await apiPost(`/api/tasks/${task.id}/dispatch`, {});
            } catch (cause) {
              setError(cause instanceof Error ? cause.message : String(cause));
            }
          }
          await load();
        }}
      />

      <SwarmModal
        task={swarmTask}
        onClose={() => setSwarmTask(null)}
        onCreated={async () => {
          await load();
        }}
      />

      <LogDrawer
        task={logTask}
        initialRunId={initialRunId}
        onClose={() => {
          setLogTask(null);
          setInitialRunId(null);
        }}
        lastRunEvent={lastRunEvent}
      />
    </>
  );
}
