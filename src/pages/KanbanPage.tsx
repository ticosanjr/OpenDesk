import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  AlertTriangle,
  ArrowRightLeft,
  CheckCircle2,
  FileText,
  GripVertical,
  ListChecks,
  Loader2,
  Pause,
  Pencil,
  Play,
  Plus,
  RotateCw,
  Search,
  Square,
  XCircle,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Modal } from "@/components/ui/modal";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import { toast } from "@/store/toast-store";
import { RunLogView } from "@/components/runs/RunLogView";
import { links } from "@/lib/links";
import { EntityLink } from "@/components/ui/entity-link";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet, apiPatch, apiPost, apiPut } from "@/lib/api";
import type {
  DispatchDecision,
  QueueJob,
  QueueStatus,
  Task,
  TaskDiagnostics,
  TaskRun,
} from "@/lib/types";
import { formatUsd, relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import { confirmAction } from "@/store/confirm-store";

const COLUMNS = [
  { id: "BACKLOG", label: "Backlog", tone: "default" as const },
  { id: "PLAN", label: "Plan", tone: "blue" as const },
  { id: "BUILD", label: "Build", tone: "purple" as const },
  { id: "REVIEW", label: "Review / Audit", tone: "yellow" as const },
  { id: "DONE", label: "Done", tone: "green" as const },
];

const PRIORITY_LABEL: Record<number, string> = {
  1: "baixa",
  2: "normal",
  3: "alta",
  4: "crítica",
};

const PRIORITY_TONE: Record<number, "default" | "yellow" | "red"> = {
  1: "default",
  2: "default",
  3: "yellow",
  4: "red",
};

export function KanbanPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const queue = useAppStore((state) => state.queue);
  const lastTaskEvent = useAppStore((state) => state.lastTaskEvent);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const taskDialogOpen = useUiStore((state) => state.taskDialogOpen);
  const setTaskDialogOpen = useUiStore((state) => state.setTaskDialogOpen);

  const [tasks, setTasks] = useState<Task[]>([]);
  const [queueStatus, setQueueStatus] = useState<QueueStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [selectedTask, setSelectedTask] = useState<Task | null>(null);
  const [editTask, setEditTask] = useState<Task | null>(null);
  const [logTask, setLogTask] = useState<Task | null>(null);
  const [initialRunId, setInitialRunId] = useState<string | null>(null);
  const [decision, setDecision] = useState<DispatchDecision | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("");
  const [priorityFilter, setPriorityFilter] = useState("");
  const draggedRef = useRef<string | null>(null);
  const [searchParams, setSearchParams] = useSearchParams();

  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);
  const workspaceQuery = useMemo(
    () => (activeWorkspaceId ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}` : ""),
    [activeWorkspaceId],
  );

  const filteredTasks = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return tasks.filter((task) => {
      if (needle) {
        const haystack = `${task.title} ${task.description} ${task.tags_json} ${task.assigned_agent ?? ""}`.toLowerCase();
        if (!haystack.includes(needle)) return false;
      }
      if (agentFilter && (task.assigned_agent ?? "") !== agentFilter) return false;
      if (priorityFilter && String(task.priority) !== priorityFilter) return false;
      return true;
    });
  }, [agentFilter, priorityFilter, query, tasks]);

  const boardAgents = useMemo(
    () =>
      Array.from(
        new Set(tasks.map((task) => task.assigned_agent).filter(Boolean)),
      ).sort() as string[],
    [tasks],
  );

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setTasks([]);
      setQueueStatus(null);
      return;
    }
    setLoading(true);
    try {
      const [taskList, jobs, status] = await Promise.all([
        apiGet<Task[]>(`/api/tasks${workspaceQuery}`),
        apiGet<QueueJob[]>(`/api/queue${workspaceQuery}`),
        apiGet<QueueStatus>(`/api/queue/status${workspaceQuery}`),
      ]);
      setTasks(taskList);
      setQueueStatus(status);
      useAppStore.setState({ queue: jobs });
      setError(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, [activeWorkspaceId, workspaceQuery]);

  useEffect(() => {
    void load();
  }, [load, lastTaskEvent?.task_id, lastRunEvent?.run_id, lastRunEvent?.status]);

  useEffect(() => {
    if (taskDialogOpen) {
      setCreateOpen(true);
      setTaskDialogOpen(false);
    }
  }, [taskDialogOpen, setTaskDialogOpen]);

  // Deep links from the dashboard: /kanban?task=<id>&run=<id>
  useEffect(() => {
    const taskId = searchParams.get("task");
    if (!taskId || tasks.length === 0) return;
    const task = tasks.find((item) => item.id === taskId);
    if (task) {
      setSelectedTask(task);
      const runId = searchParams.get("run");
      if (runId) {
        setInitialRunId(runId);
        setLogTask(task);
      }
      setSearchParams({}, { replace: true });
    }
  }, [tasks, searchParams, setSearchParams]);

  async function dispatch(task: Task) {
    setError(null);
    try {
      const response = await apiPost<{ decision: DispatchDecision }>(
        `/api/tasks/${task.id}/dispatch`,
        {},
      );
      setDecision(response.decision);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

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

  async function transitionTask(task: Task, stage: string) {
    if (!stage || stage === task.column_stage) return;
    const running = task.phase_status === "queued" || task.phase_status === "running";
    if (
      stage !== "DONE" &&
      stage !== "BACKLOG" &&
      !running &&
      !(await confirmAction({
        title: `Mover para ${stage}?`,
        description:
          "Isso pode iniciar uma execução no OpenCode e consumir budget. A coluna é persistida antes do disparo.",
        confirmLabel: "Mover e executar",
      }))
    ) {
      return;
    }
    if (
      stage === "BACKLOG" &&
      running &&
      !(await confirmAction({
        title: "Cancelar a execução atual?",
        description: "A tarefa volta para Backlog e o run em andamento é interrompido.",
        confirmLabel: "Cancelar execução",
        tone: "destructive",
      }))
    ) {
      return;
    }
    setError(null);
    // Optimistic move so the card does not jump back while the API responds.
    setTasks((current) =>
      current.map((item) =>
        item.id === task.id ? { ...item, column_stage: stage } : item,
      ),
    );
    try {
      const response = await apiPost<{ error?: string | null }>(
        `/api/tasks/${task.id}/transition`,
        { column_stage: stage },
      );
      if (response.error) {
        setError(response.error);
        setNotice(`Coluna alterada para ${stage}; a execução não iniciou.`);
      } else {
        setNotice(`Coluna alterada para ${stage}.`);
      }
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      await load();
    }
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

  async function jobAction(id: string, action: "cancel" | "retry") {
    try {
      await apiPost(`/api/queue/${id}/${action}`);
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader
          eyebrow="gestão de tarefas"
          title="Kanban & Fila"
          description="Crie um workspace para planejar, executar e auditar tarefas."
        />
        <EmptyState
          icon={ListChecks}
          title="Nenhum workspace ativo"
          description="O Kanban opera sobre o workspace selecionado."
          action={
            <Button size="sm" onClick={() => setWorkspaceDialogOpen(true)}>
              <Plus size={14} />
              Criar workspace
            </Button>
          }
        />
      </PageBody>
    );
  }

  return (
    <PageBody>
      <PageHeader
        eyebrow="gestão de tarefas"
        title="Kanban & Fila"
        description={`${workspace?.name ?? "Workspace"} · tarefas passam por Plan, Build e Audit; a fila executa no OpenCode com budget guard.`}
        actions={
          <>
            {queueStatus ? (
              <Badge variant={queueStatus.paused ? "yellow" : "green"}>
                {queueStatus.paused ? "fila pausada" : "fila ativa"}
              </Badge>
            ) : null}
            <Button size="sm" onClick={() => setCreateOpen(true)}>
              <Plus size={15} />
              Nova tarefa
            </Button>
          </>
        }
      />

      {error ? (
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}
      {notice ? (
        <p className="flex items-center gap-2 rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[11px] text-emerald-300">
          <CheckCircle2 size={13} />
          {notice}
          <button
            type="button"
            className="ml-auto text-[10px] text-emerald-300/70 hover:text-emerald-200"
            onClick={() => setNotice(null)}
          >
            fechar
          </button>
        </p>
      ) : null}

      {decision ? (
        <div className="flex flex-wrap items-center gap-2 rounded-xl border border-blue-500/20 bg-blue-500/[0.06] px-4 py-3 text-[11px] text-blue-200">
          <Badge variant="blue">{decision.phase}</Badge>
          <span className="font-medium">
            {decision.selected_agent} · {decision.selected_model} · {decision.effort}
          </span>
          <span className="text-blue-300/70">{decision.rationale}</span>
          <button
            type="button"
            className="ml-auto text-[10px] text-blue-300/60 hover:text-blue-200"
            onClick={() => setDecision(null)}
          >
            fechar
          </button>
        </div>
      ) : null}

      <div className="flex flex-wrap items-center gap-2 rounded-xl border border-zinc-800/70 bg-zinc-900/20 px-3 py-2">
        <div className="flex h-8 min-w-[220px] flex-1 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5">
          <Search size={13} className="text-zinc-600" />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Buscar por título, descrição, tag ou agente…"
            className="h-full flex-1 bg-transparent text-[11px] outline-none placeholder:text-zinc-600"
          />
        </div>
        <select
          value={agentFilter}
          onChange={(event) => setAgentFilter(event.target.value)}
          className="h-8 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2 text-[10px] text-zinc-300"
          aria-label="Filtrar por agente"
        >
          <option value="">todos os agentes</option>
          {boardAgents.map((agent) => (
            <option key={agent} value={agent}>
              @{agent}
            </option>
          ))}
        </select>
        <select
          value={priorityFilter}
          onChange={(event) => setPriorityFilter(event.target.value)}
          className="h-8 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2 text-[10px] text-zinc-300"
          aria-label="Filtrar por prioridade"
        >
          <option value="">todas as prioridades</option>
          {[4, 3, 2, 1].map((priority) => (
            <option key={priority} value={priority}>
              {priority} — {PRIORITY_LABEL[priority]}
            </option>
          ))}
        </select>
        <Badge variant="default">{filteredTasks.length} de {tasks.length}</Badge>
        {(query || agentFilter || priorityFilter) ? (
          <button
            type="button"
            onClick={() => {
              setQuery("");
              setAgentFilter("");
              setPriorityFilter("");
            }}
            className="text-[10px] text-zinc-500 hover:text-zinc-200"
          >
            limpar filtros
          </button>
        ) : null}
        <label className="ml-auto flex items-center gap-2 text-[10px] text-zinc-500">
          concorrência
          <select
            value={queueStatus?.concurrency ?? 1}
            onChange={(event) => void setConcurrency(Number(event.target.value))}
            className="h-8 rounded-lg border border-zinc-800 bg-zinc-900/50 px-2 text-[10px] text-zinc-300"
            aria-label="Concorrência da fila"
          >
            {[1, 2, 3, 4].map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="-mx-1 flex gap-3 overflow-x-auto px-1 pb-2">
        {COLUMNS.map((column) => {
          const columnTasks = filteredTasks.filter((task) => task.column_stage === column.id);
          return (
            <Card
              key={column.id}
              data-column={column.id}
              className="flex min-h-[440px] w-[280px] shrink-0 flex-col"
              onDragOver={(event) => event.preventDefault()}
              onDrop={(event) => {
                const taskId =
                  event.dataTransfer.getData("text/task-id") ||
                  draggedRef.current;
                draggedRef.current = null;
                const task = tasks.find((item) => item.id === taskId);
                if (task) void transitionTask(task, column.id);
              }}
            >
              <CardHeader className="px-3.5 py-3">
                <CardTitle className="flex items-center gap-2 text-[12px]">
                  <span className="h-1.5 w-1.5 rounded-full bg-zinc-600" />
                  {column.label}
                </CardTitle>
                <Badge variant={column.tone}>{columnTasks.length}</Badge>
              </CardHeader>
              <CardContent className="flex-1 space-y-2 p-2.5">
                {columnTasks.length === 0 ? (
                  <div className="rounded-lg border border-dashed border-zinc-800/80 bg-zinc-950/30 px-3 py-8 text-center">
                    <GripVertical size={15} className="mx-auto text-zinc-700" />
                    <p className="mt-2 text-[10px] text-zinc-600">Arraste uma tarefa</p>
                  </div>
                ) : (
                  columnTasks.map((task) => (
                    <div
                      key={task.id}
                      role="button"
                      tabIndex={0}
                      aria-label={`Abrir tarefa ${task.title}`}
                      className={cn(
                        "rounded-lg border bg-zinc-950/50 p-2.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-blue-500/60",
                        task.phase_status?.includes("failed")
                          ? "border-red-500/40"
                          : "border-zinc-800/80",
                      )}
                      onClick={() => setSelectedTask(task)}
                      onKeyDown={(event) => {
                        if (event.key === "Enter" || event.key === " ") {
                          event.preventDefault();
                          setSelectedTask(task);
                        }
                      }}
                    >
                      <div className="flex items-start justify-between gap-2">
                        <div className="flex min-w-0 items-start gap-1.5">
                          <span
                            draggable
                            title="Arrastar tarefa"
                            onDragStart={(event) => {
                              draggedRef.current = task.id;
                              event.dataTransfer.setData("text/task-id", task.id);
                              event.dataTransfer.effectAllowed = "move";
                            }}
                            onDragEnd={() => {
                              // Keep the ref until the drop handler runs; some
                              // browsers fire dragend before drop.
                              window.setTimeout(() => {
                                draggedRef.current = null;
                              }, 250);
                            }}
                            onClick={(event) => event.stopPropagation()}
                            className="mt-0.5 cursor-grab text-zinc-600 active:cursor-grabbing"
                          >
                            <GripVertical size={12} />
                          </span>
                          <p className="text-[11px] font-medium leading-snug text-zinc-200">
                            {task.title}
                          </p>
                        </div>
                        <Badge variant={PRIORITY_TONE[task.priority]}>
                          {PRIORITY_LABEL[task.priority] ?? task.priority}
                        </Badge>
                      </div>
                      <p className="mt-1 line-clamp-2 text-[9px] leading-relaxed text-zinc-600">
                        {task.description}
                      </p>
                      <div className="mt-2 flex flex-wrap items-center gap-1">
                        {task.assigned_agent ? (
                          <EntityLink
                            to={links.agent(
                              task.assigned_agent,
                              "workspace",
                              task.workspace_id,
                            )}
                            chip
                          >
                            @{task.assigned_agent}
                          </EntityLink>
                        ) : null}
                        <Badge variant="default">{task.effort_level}</Badge>
                        {task.phase_status ? (
                          <Badge
                            variant={
                              task.phase_status.includes("failed")
                                ? "red"
                                : task.phase_status.includes("approval")
                                  ? "yellow"
                                  : "default"
                            }
                          >
                            {task.phase_status}
                          </Badge>
                        ) : null}
                      </div>
                      <div className="mt-2 flex flex-wrap items-center gap-1.5 border-t border-zinc-800/60 pt-2">
                        <select
                          aria-label={`Mover ${task.title}`}
                          value=""
                          onClick={(event) => event.stopPropagation()}
                          onChange={(event) => {
                            event.stopPropagation();
                            void transitionTask(task, event.target.value);
                          }}
                          className="h-6 w-full rounded-md border border-zinc-800 bg-zinc-900/60 px-1 text-[9px] text-zinc-300 outline-none"
                        >
                          <option value="">mover…</option>
                          {COLUMNS.filter((item) => item.id !== task.column_stage).map(
                            (item) => (
                              <option key={item.id} value={item.id}>
                                {item.label}
                              </option>
                            ),
                          )}
                        </select>
                        {task.phase_status === "awaiting_approval" ? (
                          <Button
                            size="sm"
                            className="h-6 px-1.5 text-[9px]"
                            onClick={(event) => {
                              event.stopPropagation();
                              void approve(task);
                            }}
                          >
                            <CheckCircle2 size={11} />
                            Aprovar plano
                          </Button>
                        ) : (
                          <Button
                            variant="outline"
                            size="sm"
                            className="h-6 px-1.5 text-[9px]"
                            onClick={(event) => {
                              event.stopPropagation();
                              void dispatch(task);
                            }}
                          >
                            <Play size={11} />
                            Executar
                          </Button>
                        )}
                        <Button
                          variant="ghost"
                          size="sm"
                          className="h-6 px-1.5 text-[9px]"
                          onClick={(event) => {
                            event.stopPropagation();
                            setLogTask(task);
                          }}
                        >
                          <FileText size={11} />
                          Logs
                        </Button>
                      </div>
                    </div>
                  ))
                )}
              </CardContent>
            </Card>
          );
        })}
      </div>

      <Card>
        <CardHeader>
          <div>
            <CardTitle className="flex items-center gap-2">
              <ListChecks size={15} className="text-zinc-500" />
              Controlador da fila
            </CardTitle>
            <p className="mt-1 text-xs text-zinc-500">
              {queueStatus?.running ?? 0} rodando · {queueStatus?.waiting ?? 0} aguardando
              {queueStatus?.pause_reason ? ` · ${queueStatus.pause_reason}` : ""}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <label className="flex items-center gap-2 text-[10px] text-zinc-500">
              concorrência
              <select
                value={queueStatus?.concurrency ?? 1}
                onChange={(event) => void setConcurrency(Number(event.target.value))}
                className="h-7 rounded-md border border-zinc-800 bg-zinc-900/60 px-2 text-[10px] text-zinc-200 outline-none focus:border-blue-500/60"
              >
                {[1, 2, 3, 4].map((value) => (
                  <option key={value} value={value}>
                    {value}
                  </option>
                ))}
              </select>
            </label>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void toggleQueue(!(queueStatus?.paused ?? false))}
            >
              {queueStatus?.paused ? <Play size={13} /> : <Pause size={13} />}
              {queueStatus?.paused ? "Retomar" : "Pausar"}
            </Button>
            <Button variant="ghost" size="sm" onClick={() => void load()} disabled={loading}>
              <RotateCw size={13} className={loading ? "animate-spin" : undefined} />
              Atualizar
            </Button>
          </div>
        </CardHeader>
        <CardContent>
          {queue.length === 0 ? (
            <EmptyState
              icon={ListChecks}
              title="Fila vazia"
              description="Execute uma tarefa para enfileirar um job. A fila persiste no SQLite e recupera jobs interrompidos após reinício."
            />
          ) : (
            <div className="space-y-2">
              {queue.map((job) => (
                <div
                  key={job.id}
                  className="flex flex-wrap items-center gap-3 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2.5"
                >
                  <Badge
                    variant={
                      job.status === "RUNNING"
                        ? "blue"
                        : job.status === "FAILED"
                          ? "red"
                          : job.status === "COMPLETED"
                            ? "green"
                            : job.status === "CANCELLED"
                              ? "default"
                              : "yellow"
                    }
                  >
                    {job.status}
                  </Badge>
                  {job.job_kind && job.job_kind !== "task" ? (
                    <Badge variant="purple">{job.job_kind}</Badge>
                  ) : null}
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[11px] text-zinc-200">
                      {job.title ?? job.id}
                    </p>
                    <p className="truncate font-mono text-[9px] text-zinc-600">
                      {job.resolved_mode} · {job.resolved_agent} · {job.resolved_model} ·{" "}
                      {job.resolved_effort}
                      {(job.retry_count ?? 0) > 0
                        ? ` · retry ${job.retry_count}/${job.max_retries ?? "?"}`
                        : ""}
                    </p>
                    {job.error_message ? (
                      <p className="mt-0.5 truncate text-[9px] text-red-300/80">
                        {job.error_message}
                      </p>
                    ) : null}
                  </div>
                  <span className="font-mono text-[9px] text-zinc-600">
                    {relativeTime(job.enqueued_at)}
                  </span>
                  {job.status === "RUNNING" || job.status === "QUEUED" ? (
                    <Button
                      variant="destructive"
                      size="sm"
                      className="h-6 px-1.5 text-[9px]"
                      onClick={() => void jobAction(job.id, "cancel")}
                    >
                      <Square size={10} />
                      Cancelar
                    </Button>
                  ) : null}
                  {job.status === "FAILED" || job.status === "CANCELLED" ? (
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-6 px-1.5 text-[9px]"
                      onClick={() => void jobAction(job.id, "retry")}
                    >
                      <RotateCw size={10} />
                      Retry
                    </Button>
                  ) : null}
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      <CreateTaskModal
        open={createOpen}
        workspaceId={activeWorkspaceId}
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
        onDispatch={(task) => void dispatch(task)}
        onLogs={(task) => {
          setLogTask(task);
          setInitialRunId(null);
          setSelectedTask(null);
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

      <LogDrawer
        task={logTask}
        initialRunId={initialRunId}
        onClose={() => {
          setLogTask(null);
          setInitialRunId(null);
        }}
        lastRunEvent={lastRunEvent}
      />
    </PageBody>
  );
}

function CreateTaskModal({
  open,
  workspaceId,
  onClose,
  onCreated,
}: {
  open: boolean;
  workspaceId: string;
  onClose: () => void;
  onCreated: () => Promise<void>;
}) {
  return (
    <TaskFormModal
      open={open}
      mode="create"
      task={null}
      workspaceId={workspaceId}
      onClose={onClose}
      onSaved={async () => {
        await onCreated();
      }}
    />
  );
}

function EditTaskModal({
  task,
  onClose,
  onSaved,
}: {
  task: Task | null;
  onClose: () => void;
  onSaved: (task: Task, rerun: boolean) => Promise<void>;
}) {
  return (
    <TaskFormModal
      open={!!task}
      mode="edit"
      task={task}
      workspaceId={task?.workspace_id ?? null}
      onClose={onClose}
      onSaved={async (saved, rerun) => {
        await onSaved(saved, rerun);
      }}
    />
  );
}

function TaskFormModal({
  open,
  mode,
  task,
  workspaceId,
  onClose,
  onSaved,
}: {
  open: boolean;
  mode: "create" | "edit";
  task: Task | null;
  workspaceId: string | null;
  onClose: () => void;
  onSaved: (task: Task, rerun: boolean) => Promise<void>;
}) {
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [executionMode, setExecutionMode] = useState("AUTO_DSH");
  const [priority, setPriority] = useState(2);
  const [tags, setTags] = useState("");
  const [agent, setAgent] = useState("");
  const [model, setModel] = useState("");
  const [effort, setEffort] = useState("medium");
  const [email, setEmail] = useState("");
  const [exportToGdocs, setExportToGdocs] = useState(false);
  const [saving, setSaving] = useState<"save" | "rerun" | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setError(null);
    if (task) {
      setTitle(task.title);
      setDescription(task.description);
      setExecutionMode(task.execution_mode);
      setPriority(task.priority);
      setAgent(task.assigned_agent ?? "");
      setModel(task.preferred_model ?? "");
      setEffort(task.effort_level);
      setEmail(task.notify_email ?? "");
      setExportToGdocs(task.export_to_gdocs);
      try {
        setTags((JSON.parse(task.tags_json) as string[]).join(", "));
      } catch {
        setTags("");
      }
    } else {
      setTitle("");
      setDescription("");
      setExecutionMode("AUTO_DSH");
      setPriority(2);
      setAgent("");
      setModel("");
      setEffort("medium");
      setEmail("");
      setExportToGdocs(false);
      setTags("");
    }
  }, [open, task]);

  if (!open) return null;
  const running =
    task?.phase_status === "queued" || task?.phase_status === "running";

  async function submit(rerun: boolean) {
    if (!workspaceId) return;
    if (!title.trim() || !description.trim()) {
      setError("Título e descrição são obrigatórios.");
      return;
    }
    setSaving(rerun ? "rerun" : "save");
    setError(null);
    try {
      const payload = {
        title: title.trim(),
        description: description.trim(),
        execution_mode: executionMode,
        priority,
        assigned_agent: agent.trim() || null,
        preferred_model: model.trim() || null,
        effort_level: effort,
        notify_email: email.trim() || null,
        export_to_gdocs: exportToGdocs,
        tags: tags
          .split(",")
          .map((tag) => tag.trim())
          .filter(Boolean),
      };
      if (mode === "create") {
        const created = await apiPost<Task>(
          `/api/tasks?workspace_id=${encodeURIComponent(workspaceId)}`,
          payload,
        );
        toast({
          tone: "success",
          title: "Tarefa criada",
          description: `“${created.title}” entrou no Kanban.`,
          href: links.task(created.id),
        });
        await onSaved(created, false);
      } else if (task) {
        const saved = await apiPatch<Task>(`/api/tasks/${task.id}`, {
          ...payload,
          agent_override: Boolean(agent.trim()),
          model_override: Boolean(model.trim()),
          clear_agent: !agent.trim(),
          clear_model: !model.trim(),
        });
        toast({
          tone: "success",
          title: rerun ? "Tarefa salva e reenfileirada" : "Tarefa atualizada",
          description: saved.title,
          href: links.task(saved.id),
        });
        await onSaved(saved, rerun);
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(null);
    }
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={mode === "create" ? "Nova tarefa" : "Editar tarefa"}
      description={
        mode === "create"
          ? "O roteador DSH decide Plan/Build, agente, modelo e effort. Você pode sobrescrever manualmente."
          : "Corrija agente, modelo, prompt e prioridade sem recriar o card."
      }
      className="max-w-2xl"
    >
      <div className="grid gap-3 md:grid-cols-2">
        <label className="md:col-span-2">
          <span className={labelClass}>Título</span>
          <input
            autoFocus
            value={title}
            onChange={(event) => setTitle(event.target.value)}
            placeholder="Relatório executivo de vendas"
            className={inputClass}
          />
        </label>
        <div className="md:col-span-2">
          <span className={labelClass}>Descrição / prompt</span>
          <SlashTextarea
            value={description}
            onChange={setDescription}
            workspaceId={workspaceId}
            rows={5}
            placeholder="Descreva o objetivo, as entradas e os critérios de aceite… Digite / para comandos e skills."
          />
        </div>
        <label>
          <span className={labelClass}>Modo</span>
          <select
            value={executionMode}
            onChange={(event) => setExecutionMode(event.target.value)}
            className={inputClass}
          >
            <option value="AUTO_DSH">AUTO_DSH — roteador decide</option>
            <option value="PLAN_ONLY">PLAN_ONLY — só plano</option>
            <option value="PLAN_THEN_BUILD">PLAN_THEN_BUILD — aprovar plano</option>
            <option value="BUILD_DIRECT">BUILD_DIRECT — executar direto</option>
          </select>
        </label>
        <label>
          <span className={labelClass}>Prioridade</span>
          <select
            value={priority}
            onChange={(event) => setPriority(Number(event.target.value))}
            className={inputClass}
          >
            {[4, 3, 2, 1].map((value) => (
              <option key={value} value={value}>
                {value} — {PRIORITY_LABEL[value]}
              </option>
            ))}
          </select>
        </label>
        <div>
          <span className={labelClass}>Agente (opcional)</span>
          <AgentCombobox
            value={agent}
            onChange={(value) => setAgent(value)}
            workspaceId={workspaceId}
            placeholder="automático (router)"
          />
        </div>
        <div>
          <span className={labelClass}>Modelo (opcional)</span>
          <ModelCombobox
            value={model}
            onChange={setModel}
            placeholder="digite para filtrar (ex.: deepseek, glm, opencode-go/…)"
          />
        </div>
        <label>
          <span className={labelClass}>Effort</span>
          <select
            value={effort}
            onChange={(event) => setEffort(event.target.value)}
            className={inputClass}
          >
            <option value="low">low</option>
            <option value="medium">medium</option>
            <option value="high">high</option>
          </select>
        </label>
        <label>
          <span className={labelClass}>Tags (vírgula)</span>
          <input
            value={tags}
            onChange={(event) => setTags(event.target.value)}
            placeholder="roteiro, code, auditoria"
            className={inputClass}
          />
        </label>
        <label>
          <span className={labelClass}>E-mail de notificação</span>
          <input
            value={email}
            onChange={(event) => setEmail(event.target.value)}
            placeholder="ops@empresa.com"
            className={inputClass}
          />
        </label>
        <label className="flex items-end gap-2 pb-2 text-[11px] text-zinc-400">
          <input
            type="checkbox"
            checked={exportToGdocs}
            onChange={(event) => setExportToGdocs(event.target.checked)}
            className="accent-blue-500"
          />
          exportar para Google Docs
        </label>
      </div>
      {running ? (
        <p className="mt-3 rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-200">
          Cancele a execução atual antes de salvar alterações.
        </p>
      ) : null}
      {error ? (
        <p className="mt-3 rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[11px] text-red-300">
          {error}
        </p>
      ) : null}
      <div className="mt-4 flex flex-wrap justify-end gap-2 border-t border-zinc-800/70 pt-4">
        <Button variant="ghost" size="sm" onClick={onClose}>
          Cancelar
        </Button>
        {mode === "create" ? (
          <Button size="sm" onClick={() => void submit(false)} disabled={saving !== null}>
            {saving ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
            Criar tarefa
          </Button>
        ) : (
          <>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void submit(false)}
              disabled={saving !== null || running}
            >
              {saving === "save" ? <Loader2 size={14} className="animate-spin" /> : null}
              Salvar
            </Button>
            <Button
              size="sm"
              onClick={() => void submit(true)}
              disabled={saving !== null || running}
            >
              {saving === "rerun" ? (
                <Loader2 size={14} className="animate-spin" />
              ) : (
                <ArrowRightLeft size={14} />
              )}
              Salvar e reexecutar
            </Button>
          </>
        )}
      </div>
    </Modal>
  );
}

function TaskDetailsModal({
  task,
  onClose,
  onEdit,
  onApprove,
  onDispatch,
  onLogs,
}: {
  task: Task | null;
  onClose: () => void;
  onEdit: (task: Task) => void;
  onApprove: (task: Task) => void;
  onDispatch: (task: Task) => void;
  onLogs: (task: Task) => void;
}) {
  const [diagnostics, setDiagnostics] = useState<TaskDiagnostics | null>(null);
  const [diagnosticsError, setDiagnosticsError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

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

  const running =
    task?.phase_status === "queued" || task?.phase_status === "running";
  const failed =
    task?.phase_status?.includes("failed") ||
    diagnostics?.error !== null && diagnostics?.error !== undefined;

  return (
    <Modal
      open={!!task}
      onClose={onClose}
      title={task?.title ?? "Tarefa"}
      description={
        task
          ? `${task.execution_mode} · ${task.effort_level} · ${task.assigned_agent ?? "router"}`
          : undefined
      }
      className="max-w-3xl"
    >
      {task ? (
        <div className="space-y-4">
          <div className="flex flex-wrap gap-2">
            <Badge variant="default">{task.column_stage}</Badge>
            <Badge variant="default">prioridade {task.priority}</Badge>
            {task.phase_status ? (
              <Badge variant={failed ? "red" : running ? "blue" : "yellow"}>
                {task.phase_status}
              </Badge>
            ) : null}
            {task.assigned_agent ? (
              <EntityLink
                to={links.agent(task.assigned_agent, "workspace", task.workspace_id)}
                chip
              >
                @{task.assigned_agent}
              </EntityLink>
            ) : null}
            {task.preferred_model ? (
              <Badge variant="default">{task.preferred_model}</Badge>
            ) : null}
            {task.output_dir_path ? (
              <Badge variant="blue">{task.output_dir_path}</Badge>
            ) : null}
          </div>

          {running ? (
            <p className="rounded-lg border border-blue-500/20 bg-blue-500/10 px-3 py-2 text-[10px] text-blue-200">
              Esta tarefa está em execução. Cancele no controlador da fila antes de
              editar agente, modelo ou prompt.
            </p>
          ) : null}

          {diagnostics?.error ? (
            <div className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-3 text-red-200">
              <div className="flex flex-wrap items-center gap-2">
                <AlertTriangle size={14} />
                <span className="text-[11px] font-semibold">
                  O que falhou: {diagnostics.error.code}
                </span>
                <Badge variant="red">{diagnostics.error.source}</Badge>
                <button
                  type="button"
                  onClick={() => void copyError()}
                  className="ml-auto text-[10px] text-red-300/80 underline-offset-2 hover:underline"
                >
                  {copied ? "copiado" : "copiar erro"}
                </button>
              </div>
              <p className="mt-2 whitespace-pre-wrap font-mono text-[10px] leading-relaxed">
                {diagnostics.error.message}
              </p>
              <p className="mt-2 text-[10px] leading-relaxed text-red-100/80">
                <strong>Como resolver:</strong> {diagnostics.error.hint}
              </p>
            </div>
          ) : diagnosticsError ? (
            <p className="rounded-lg border border-amber-500/20 bg-amber-500/10 px-3 py-2 text-[10px] text-amber-200">
              Não foi possível carregar o diagnóstico: {diagnosticsError}
            </p>
          ) : null}

          <div>
            <p className={labelClass}>Descrição</p>
            <p className="whitespace-pre-wrap rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 text-[11px] leading-relaxed text-zinc-400">
              {task.description}
            </p>
          </div>
          {task.plan_markdown ? (
            <div>
              <p className={labelClass}>Plano</p>
              <pre className="max-h-[320px] overflow-auto whitespace-pre-wrap rounded-lg border border-zinc-800/70 bg-zinc-950/40 p-3 font-mono text-[10px] leading-relaxed text-zinc-300">
                {task.plan_markdown}
              </pre>
            </div>
          ) : null}

          {diagnostics && diagnostics.runs.length > 0 ? (
            <div>
              <p className={labelClass}>Últimas execuções</p>
              <div className="space-y-1.5">
                {diagnostics.runs.slice(0, 3).map((run) => (
                  <Link
                    key={run.id}
                    to={links.run(run.id)}
                    className="flex flex-wrap items-center gap-2 rounded-lg border border-zinc-800/70 bg-zinc-950/40 px-3 py-2 text-[10px] transition-colors hover:border-blue-500/30 hover:bg-zinc-900/50"
                  >
                    <Badge variant={runVariant(run.status)}>{run.status}</Badge>
                    <span className="font-mono text-zinc-400">{run.phase ?? "run"}</span>
                    <span className="text-zinc-500">
                      {run.agent_name} · {formatUsd(run.estimated_cost_usd)} ·{" "}
                      {Math.round(run.duration_ms / 1000)}s
                    </span>
                    <span className="ml-auto text-zinc-600">
                      {relativeTime(run.started_at)}
                    </span>
                    {run.error_message ? (
                      <p className="w-full truncate font-mono text-[9px] text-red-300/80">
                        {run.error_message}
                      </p>
                    ) : null}
                  </Link>
                ))}
              </div>
            </div>
          ) : null}

          <div className="flex flex-wrap justify-end gap-2 border-t border-zinc-800/70 pt-4">
            <Button variant="ghost" size="sm" onClick={onClose}>
              Fechar
            </Button>
            <Button variant="outline" size="sm" onClick={() => onEdit(task)}>
              <Pencil size={13} />
              Editar
            </Button>
            <Button variant="outline" size="sm" onClick={() => onLogs(task)}>
              <FileText size={13} />
              Logs
            </Button>
            {task.phase_status === "awaiting_approval" ? (
              <Button size="sm" onClick={() => onApprove(task)}>
                <CheckCircle2 size={13} />
                Aprovar plano e iniciar Build
              </Button>
            ) : (
              <Button size="sm" onClick={() => onDispatch(task)}>
                <Play size={13} />
                Executar agora
              </Button>
            )}
          </div>
        </div>
      ) : null}
    </Modal>
  );
}

function runVariant(status: string): "green" | "red" | "blue" | "yellow" | "default" {
  if (status === "SUCCESS") return "green";
  if (status === "FAILED") return "red";
  if (status === "RUNNING") return "blue";
  if (status === "CANCELLED") return "yellow";
  return "default";
}


function LogDrawer({
  task,
  initialRunId,
  onClose,
  lastRunEvent,
}: {
  task: Task | null;
  initialRunId?: string | null;
  onClose: () => void;
  lastRunEvent: { run_id: string; status: string } | null;
}) {
  const [runs, setRuns] = useState<TaskRun[]>([]);
  const [selectedRun, setSelectedRun] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!task) return;
    let cancelled = false;
    setLoading(true);
    apiGet<TaskRun[]>(
      `/api/runs?task_id=${encodeURIComponent(task.id)}&workspace_id=${encodeURIComponent(task.workspace_id)}`,
    )
      .then((items) => {
        if (cancelled) return;
        setRuns(items);
        setSelectedRun((current) => {
          if (initialRunId && items.some((item) => item.id === initialRunId)) {
            return initialRunId;
          }
          return current ?? items[0]?.id ?? null;
        });
      })
      .catch((cause) =>
        setError(cause instanceof Error ? cause.message : String(cause)),
      )
      .finally(() => setLoading(false));
    return () => {
      cancelled = true;
    };
  }, [task, initialRunId, lastRunEvent?.run_id, lastRunEvent?.status]);

  if (!task) return null;

  return (
    <div className="fixed inset-0 z-40 flex justify-end">
      <button
        type="button"
        aria-label="Fechar logs"
        className="absolute inset-0 bg-black/60"
        onClick={onClose}
      />
      <aside className="relative flex h-full w-full max-w-5xl flex-col border-l border-zinc-800 bg-zinc-950 shadow-2xl">
        <div className="flex items-center justify-between border-b border-zinc-800/70 px-4 py-3">
          <div className="min-w-0">
            <p className="truncate text-[12px] font-medium text-zinc-200">
              {task.title}
            </p>
            <p className="font-mono text-[9px] text-zinc-600">
              {task.column_stage} · {task.phase_status ?? "sem fase"}
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            {selectedRun ? (
              <Link
                to={links.run(selectedRun)}
                className="rounded-lg border border-zinc-800 px-2.5 py-1 text-[10px] text-zinc-400 hover:border-zinc-700 hover:text-zinc-200"
              >
                abrir página
              </Link>
            ) : null}
            <Button variant="ghost" size="sm" onClick={onClose}>
              <XCircle size={14} />
              Fechar
            </Button>
          </div>
        </div>
        <div className="flex min-h-0 flex-1">
          <div className="w-56 shrink-0 overflow-y-auto border-r border-zinc-800/70 p-2">
            {loading && runs.length === 0 ? (
              <p className="p-2 text-[10px] text-zinc-600">Carregando…</p>
            ) : error ? (
              <p className="p-2 text-[10px] text-red-300">{error}</p>
            ) : runs.length === 0 ? (
              <p className="p-2 text-[10px] text-zinc-600">Nenhum run.</p>
            ) : (
              runs.map((run) => (
                <button
                  key={run.id}
                  type="button"
                  onClick={() => setSelectedRun(run.id)}
                  className={cn(
                    "mb-1 w-full rounded-lg px-2 py-1.5 text-left",
                    selectedRun === run.id
                      ? "bg-zinc-800 text-zinc-100"
                      : "text-zinc-500 hover:bg-zinc-900",
                  )}
                >
                  <div className="flex items-center justify-between gap-1">
                    <p className="truncate text-[10px]">{run.phase ?? "run"}</p>
                    <Badge variant={runVariant(run.status)}>{run.status}</Badge>
                  </div>
                  <p className="font-mono text-[8px] text-zinc-600">
                    {formatUsd(run.estimated_cost_usd)} · {relativeTime(run.started_at)}
                  </p>
                  {run.error_message ? (
                    <p className="truncate text-[8px] text-red-400/80">
                      {run.error_message}
                    </p>
                  ) : null}
                </button>
              ))
            )}
          </div>
          <div className="min-w-0 flex-1 overflow-hidden bg-zinc-950/60">
            {selectedRun ? (
              <RunLogView runId={selectedRun} compact />
            ) : (
              <EmptyState
                icon={FileText}
                title="Nenhum run selecionado"
                description="Execute a tarefa ou escolha um run na lista à esquerda."
                className="border-0 bg-transparent"
              />
            )}
          </div>
        </div>
      </aside>
    </div>
  );
}

const inputClass =
  "h-9 w-full rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 text-[12px] text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-blue-500/60";
const labelClass =
  "mb-1.5 block text-[10px] font-medium uppercase tracking-[0.12em] text-zinc-500";
