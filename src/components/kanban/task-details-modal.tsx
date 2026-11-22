import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Modal } from "@/components/ui/modal";
import { apiGet, apiPost } from "@/lib/api";
import { isTaskRunning } from "@/lib/task-state";
import type {
  Task,
  TaskDiagnostics,
  TaskInteraction,
} from "@/lib/types";
import type { OutboundSink } from "@/components/integrations/OutboundSinksCard";
import { useAppStore } from "@/store/app-store";
import { toast } from "@/store/toast-store";
import { TaskDetailActions } from "./details/task-actions";
import {
  TaskContextCard,
  TaskDescriptionSection,
  TaskDiagnosticsErrorCard,
  TaskDiagnosticsErrorNotice,
  TaskRecentRuns,
  TaskSinksCard,
  type TaskContextUsage,
} from "./details/task-detail-cards";
import {
  TaskDetailSummary,
  type TaskPermissionEntry,
} from "./details/task-detail-summary";
import { TaskTokenTotals } from "./details/token-stats";

export function TaskDetailsModal({
  task,
  onClose,
  onEdit,
  onApprove,
  onDispatch,
  onStatus,
  onWait,
  onFork,
  onSwarm,
  onLogs,
}: {
  task: Task | null;
  onClose: () => void;
  onEdit: (task: Task) => void;
  onApprove: (task: Task) => void;
  onDispatch: (task: Task) => void;
  onStatus: (task: Task, status: string) => void;
  onWait: (task: Task) => void;
  onFork: (task: Task) => void;
  onSwarm: (task: Task) => void;
  onLogs: (task: Task) => void;
}) {
  const [diagnostics, setDiagnostics] = useState<TaskDiagnostics | null>(null);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [context, setContext] = useState<TaskContextUsage | null>(null);
  const [compacting, setCompacting] = useState(false);
  const [sinks, setSinks] = useState<OutboundSink[]>([]);
  const [selectedSinks, setSelectedSinks] = useState<Set<string>>(new Set());
  const [savingSinks, setSavingSinks] = useState(false);
  const [interactions, setInteractions] = useState<TaskInteraction[]>([]);
  const [permissions, setPermissions] = useState<TaskPermissionEntry[]>([]);
  const daemon = useAppStore((state) => state.daemon);
  const navigate = useNavigate();

  const taskId = task?.id;
  const taskWorkspaceId = task?.workspace_id;

  useEffect(() => {
    if (!taskId || !taskWorkspaceId) {
      setSinks([]);
      setSelectedSinks(new Set());
      return;
    }
    let cancelled = false;
    void Promise.all([
      apiGet<{ sinks: OutboundSink[] }>(
        `/api/io/outbound?workspace_id=${encodeURIComponent(taskWorkspaceId)}`,
      ),
      apiGet<{ bindings: Array<{ sink_id: string }> }>(
        `/api/tasks/${taskId}/sinks`,
      ),
    ])
      .then(([sinkList, bindings]) => {
        if (cancelled) return;
        setSinks(sinkList.sinks);
        setSelectedSinks(new Set(bindings.bindings.map((binding) => binding.sink_id)));
      })
      .catch(() => {
        if (!cancelled) {
          setSinks([]);
          setSelectedSinks(new Set());
        }
      });
    return () => {
      cancelled = true;
    };
  }, [taskId, taskWorkspaceId]);

  async function saveSinks() {
    if (!task) return;
    setSavingSinks(true);
    try {
      await apiPost(`/api/tasks/${task.id}/sinks`, {
        sink_ids: Array.from(selectedSinks),
      });
      toast({ tone: "success", title: "Entregas automáticas atualizadas" });
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível salvar as entregas",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setSavingSinks(false);
    }
  }

  async function loadInteractions() {
    if (!taskId || !taskWorkspaceId) {
      setInteractions([]);
      setPermissions([]);
      return;
    }
    try {
      const [pendingInteractions, permissionResponse] = await Promise.all([
        apiGet<TaskInteraction[]>(`/api/tasks/${taskId}/interactions`),
        apiGet<{
          requests: Array<{
            request_id: string;
            permission_action: string;
            resources?: string | null;
            task_id?: string | null;
            run_id?: string | null;
          }>;
        }>(
          `/api/permissions?workspace_id=${encodeURIComponent(taskWorkspaceId)}`,
        ),
      ]);
      setInteractions(
        Array.isArray(pendingInteractions) ? pendingInteractions : [],
      );
      setPermissions(
        (Array.isArray(permissionResponse?.requests)
          ? permissionResponse.requests
          : []
        ).filter(
          (request) =>
            request.task_id === taskId ||
            (request.run_id && request.run_id === task?.last_run_id),
        ),
      );
    } catch {
      setInteractions([]);
      setPermissions([]);
    }
  }

  useEffect(() => {
    void loadInteractions();
    // taskId/taskWorkspaceId cover task changes; loadInteractions is intentionally
    // called only when the task identity changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [taskId, taskWorkspaceId, task?.last_run_id]);

  useEffect(() => {
    if (!task) {
      setContext(null);
      return;
    }
    let cancelled = false;
    apiGet<{
      session_id?: string | null;
      tokens_input: number;
      window: number;
      percent: number;
      messages: number;
    }>(`/api/tasks/${task.id}/context`)
      .then((value) => {
        if (!cancelled) setContext(value);
      })
      .catch(() => {
        if (!cancelled) setContext(null);
      });
    return () => {
      cancelled = true;
    };
  }, [task]);

  async function compactContext() {
    if (!task) return;
    setCompacting(true);
    try {
      await apiPost(`/api/tasks/${task.id}/compact`, {});
      toast({ tone: "success", title: "Compactação solicitada ao engine" });
      const refreshed = await apiGet<{
        session_id?: string | null;
        tokens_input: number;
        window: number;
        percent: number;
        messages: number;
      }>(`/api/tasks/${task.id}/context`);
      setContext(refreshed);
    } catch (cause) {
      toast({
        tone: "error",
        title: "Não foi possível compactar",
        description: cause instanceof Error ? cause.message : String(cause),
      });
    } finally {
      setCompacting(false);
    }
  }

  useEffect(() => {
    if (!task) {
      setDiagnostics(null);
      setDiagnosticsError(null);
      return;
    }
    let cancelled = false;
    apiGet<TaskDiagnostics>(`/api/tasks/${task.id}/diagnostics`)
      .then((value) => {
        if (!cancelled) {
          setDiagnostics(value);
          setDiagnosticsError(null);
        }
      })
      .catch((cause) => {
        if (!cancelled) {
          setDiagnosticsError(cause instanceof Error ? cause.message : String(cause));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [task]);

  async function copyError() {
    if (!diagnostics?.error) return;
    const text = [
      `Tarefa: ${task?.title ?? ""}`,
      `Código: ${diagnostics.error.code}`,
      `Erro: ${diagnostics.error.message}`,
      `Sugestão: ${diagnostics.error.hint}`,
      diagnostics.error.run_id ? `Run: ${diagnostics.error.run_id}` : "",
      diagnostics.error.job_id ? `Job: ${diagnostics.error.job_id}` : "",
    ]
      .filter(Boolean)
      .join("\n");
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard is optional.
    }
  }

  const running = task ? isTaskRunning(task) : false;
  const queued = task?.workflow_state === "QUEUED";
  const held = task?.workflow_state === "HELD";

  return (
    <Modal
      open={!!task}
      onClose={onClose}
      title={task?.title ?? "Tarefa"}
      description={
        task
          ? `${task.execution_mode} · effort ${task.effort_level} · cuidado ${task.care_level ?? "standard"} · ${task.assigned_agent ?? "router"}`
          : undefined
      }
      size="lg"
      initialFocus="dialog"
    >
      {task ? (
        <div className="space-y-4">
          <TaskDetailSummary
            task={task}
            interactions={interactions}
            permissions={permissions}
            onReload={() => void loadInteractions()}
          />

          {diagnostics ? <TaskTokenTotals runs={diagnostics.runs} /> : null}

          {task.blocked_reason ? (
            <p className="rounded-lg border border-red-500/25 bg-red-500/10 px-3 py-2 text-[13px] font-medium text-red-200">
              Motivo do bloqueio: {task.blocked_reason}
            </p>
          ) : null}

          {running ? (
            <p className="rounded-lg border border-blue-500/20 bg-blue-500/10 px-3 py-2 text-[12px] text-blue-200">
              Esta tarefa está em execução. Pare na tela de execução ou no
              controlador da fila antes de editar agente, modelo ou prompt.
            </p>
          ) : null}

          <TaskSinksCard
            sinks={sinks}
            selectedSinks={selectedSinks}
            setSelectedSinks={setSelectedSinks}
            savingSinks={savingSinks}
            onSave={() => void saveSinks()}
          />

          {context?.session_id ? (
            <TaskContextCard
              context={context}
              compacting={compacting}
              onCompact={() => void compactContext()}
            />
          ) : null}

          {diagnostics?.error ? (
            <TaskDiagnosticsErrorCard
              error={diagnostics.error}
              copied={copied}
              onCopy={() => void copyError()}
            />
          ) : diagnosticsError ? (
            <TaskDiagnosticsErrorNotice message={diagnosticsError} />
          ) : null}

          <TaskDescriptionSection task={task} />

          {diagnostics && diagnostics.runs.length > 0 ? (
            <TaskRecentRuns runs={diagnostics.runs} />
          ) : null}

          <TaskDetailActions
            task={task}
            running={running}
            queued={queued}
            held={held}
            daemonOnline={Boolean(daemon?.online)}
            runCount={diagnostics?.runs.length ?? 0}
            onOpenArtifacts={() => {
              onClose();
              navigate(`/artifacts?task=${encodeURIComponent(task.id)}`);
            }}
            onFork={onFork}
            onSwarm={onSwarm}
            onEdit={onEdit}
            onLogs={onLogs}
            onStatus={onStatus}
            onWait={onWait}
            onApprove={onApprove}
            onDispatch={onDispatch}
          />
        </div>
      ) : null}
    </Modal>
  );
}

export { LogDrawer } from "./details/log-drawer";


