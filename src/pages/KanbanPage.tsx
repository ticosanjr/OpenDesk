import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";

import {
  AlertTriangle,
  CheckCircle2,
  Columns3,
  GripVertical,
  Inbox,
  LayoutList,
  ListChecks,
  Loader2,
  Plus,
  SlidersHorizontal,
  XCircle,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { CountPill } from "@/components/ui/count-pill";
import { Modal } from "@/components/ui/modal";
import { IconLabel } from "@/components/ui/icon-label";
import { TaskCard } from "@/components/kanban/TaskCard";
import { WorkflowStatusBadge } from "@/components/ui/workflow-status";
import {
  CreateTaskModal,
  EditTaskModal,
  ForkTaskModal,
  LogDrawer,
  SwarmModal,
  TaskDetailsModal,
} from "@/components/kanban/task-modals";
import { QueueController } from "@/components/queue/QueueController";
import { PRIORITY_LABEL } from "@/lib/task-ui";
import { workflowMeta } from "@/lib/workflow";
import { EmptyState } from "@/components/ui/empty-state";
import { FilterSearch, FilterSelect } from "@/components/ui/filter-bar";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet, apiPatch, apiPost, apiPut } from "@/lib/api";
import type {
  DispatchDecision,
  QueueJob,
  Artifact,
  QueueStatus,
  Task,
} from "@/lib/types";
import { relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import {
  applyRetention,
  DEFAULT_VISIBILITY,
  loadVisibility,
  RETENTION_OPTIONS,
  saveVisibility,
  type TaskVisibility,
} from "@/lib/task-visibility";

type WorkflowColumn = {
  id: string;
  label: string;
  tone: "default" | "blue" | "green" | "yellow" | "red" | "purple";
  icon: typeof Inbox;
  states: string[];
  dropStatus: string;
  description: string;
};

const COLUMNS: WorkflowColumn[] = [
  {
    id: "QUEUE",
    label: "Fila",
    tone: "blue",
    icon: Inbox,
    states: ["READY", "QUEUED", "HELD"],
    dropStatus: "TODO",
    description: "Prontas, na fila ou em espera. Iniciam automaticamente.",
  },
  {
    id: "RUNNING",
    label: "Em execução",
    tone: "purple",
    icon: Loader2,
    states: ["RUNNING"],
    dropStatus: "IN_PROGRESS",
    description: "Rodando de verdade, com job e run ativos.",
  },
  {
    id: "ATTENTION",
    label: "Atenção",
    tone: "yellow",
    icon: AlertTriangle,
    states: ["NEEDS_APPROVAL", "WAITING_INPUT", "BLOCKED"],
    dropStatus: "BLOCKED",
    description: "Precisa de você: aprovação, pergunta, permissão ou falha.",
  },
  {
    id: "DONE",
    label: "Concluído",
    tone: "green",
    icon: CheckCircle2,
    states: ["DONE"],
    dropStatus: "DONE",
    description: "Execução concluída.",
  },
  {
    id: "CANCELLED",
    label: "Cancelado",
    tone: "default",
    icon: XCircle,
    states: ["CANCELLED"],
    dropStatus: "CANCELLED",
    description: "Execução interrompida.",
  },
];

export function KanbanPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const queue = useAppStore((state) => state.queue);
  const daemon = useAppStore((state) => state.daemon);
  const lastTaskEvent = useAppStore((state) => state.lastTaskEvent);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const taskDialogOpen = useUiStore((state) => state.taskDialogOpen);
  const setTaskDialogOpen = useUiStore((state) => state.setTaskDialogOpen);

  const [tasks, setTasks] = useState<Task[]>([]);
  const [pinned, setPinned] = useState<Record<string, number>>({});
  const [queueStatus, setQueueStatus] = useState<QueueStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [createOpen, setCreateOpen] = useState(false);
  const [selectedTask, setSelectedTask] = useState<Task | null>(null);
  const [swarmTask, setSwarmTask] = useState<Task | null>(null);
  const [editTask, setEditTask] = useState<Task | null>(null);
  const [logTask, setLogTask] = useState<Task | null>(null);
  const [initialRunId, setInitialRunId] = useState<string | null>(null);
  const [decision, setDecision] = useState<DispatchDecision | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [agentFilter, setAgentFilter] = useState("");
  const [priorityFilter, setPriorityFilter] = useState("");
  const [view, setView] = useState<"board" | "list" | "queue">("board");
  const [originFilter, setOriginFilter] = useState<"work" | "automations" | "all">(
    "work",
  );
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [blockingTask, setBlockingTask] = useState<Task | null>(null);
  const [blockReason, setBlockReason] = useState("");
  const [forkSource, setForkSource] = useState<Task | null>(null);
  const [visibility, setVisibility] = useState<TaskVisibility>(DEFAULT_VISIBILITY);
  const [visibilityOpen, setVisibilityOpen] = useState(false);
  const draggedRef = useRef<string | null>(null);
  const boardRef = useRef<HTMLDivElement | null>(null);
  const [dragOverColumn, setDragOverColumn] = useState<string | null>(null);
  const [searchParams, setSearchParams] = useSearchParams();

  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);
  const workspaceQuery = useMemo(
    () => (activeWorkspaceId ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}` : ""),
    [activeWorkspaceId],
  );

  const filteredTasks = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const filtered = tasks.filter((task) => {
      const origin = task.origin ?? "MANUAL";
      if (originFilter === "work" && origin !== "MANUAL") return false;
      if (originFilter === "automations" && origin === "MANUAL") return false;
      if (needle) {
        const haystack = `${task.title} ${task.description} ${task.tags_json} ${task.assigned_agent ?? ""}`.toLowerCase();
        if (!haystack.includes(needle)) return false;
      }
      if (agentFilter && (task.assigned_agent ?? "") !== agentFilter) return false;
      if (priorityFilter && String(task.priority) !== priorityFilter) return false;
      return true;
    });
    return applyRetention(filtered, visibility);
  }, [agentFilter, originFilter, priorityFilter, query, tasks, visibility]);

  const originCounts = useMemo(() => {
    const manual = tasks.filter((task) => (task.origin ?? "MANUAL") === "MANUAL").length;
    return { manual, automations: tasks.length - manual };
  }, [tasks]);

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
      const [taskList, jobs, status, pinnedArtifacts] = await Promise.all([
        apiGet<Task[]>(`/api/tasks${workspaceQuery}`),
        apiGet<QueueJob[]>(`/api/queue${workspaceQuery}`),
        apiGet<QueueStatus>(`/api/queue/status${workspaceQuery}`),
        apiGet<Artifact[]>(`/api/artifacts/pinned${workspaceQuery}`).catch(() => []),
      ]);
      setTasks(taskList);
      setPinned(
        pinnedArtifacts.reduce<Record<string, number>>((accumulator, artifact) => {
          if (artifact.task_id) {
            accumulator[artifact.task_id] = (accumulator[artifact.task_id] ?? 0) + 1;
          }
          return accumulator;
        }, {}),
      );
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
    setVisibility(loadVisibility(activeWorkspaceId));
  }, [activeWorkspaceId]);

  function updateVisibility(next: TaskVisibility) {
    setVisibility(next);
    saveVisibility(activeWorkspaceId, next);
  }

  useEffect(() => {
    if (taskDialogOpen) {
      setCreateOpen(true);
      setTaskDialogOpen(false);
    }
  }, [taskDialogOpen, setTaskDialogOpen]);

  useEffect(() => {
    const requested = searchParams.get("view");
    if (requested === "queue" || requested === "board" || requested === "list") {
      setView(requested);
      const next = new URLSearchParams(searchParams);
      next.delete("view");
      setSearchParams(next, { replace: true });
    }
  }, [searchParams, setSearchParams]);

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

  if (!activeWorkspaceId) {
    return (
      <PageBody>
        <PageHeader
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
        <div className="flex items-center gap-1 rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5">
          {([
            ["board", Columns3, "Board"],
            ["list", LayoutList, "Lista"],
            ["queue", ListChecks, "Fila"],
          ] as const).map(([value, Icon, label]) => (
            <button
              key={value}
              type="button"
              onClick={() => setView(value)}
              className={cn(
                "flex items-center gap-1 rounded-md px-2 py-1 text-[10px] transition-colors",
                view === value
                  ? "workspace-tab-active"
                  : "text-zinc-500 hover:text-zinc-200",
              )}
              title={label}
              aria-pressed={view === value}
            >
              <Icon size={12} />
              {label}
            </button>
          ))}
        </div>
        {view !== "queue" ? (
          <div className="flex items-center gap-1 rounded-lg border border-zinc-800 bg-zinc-950/40 p-0.5">
            {([
              ["work", `Trabalho · ${originCounts.manual}`],
              ["automations", `Automações · ${originCounts.automations}`],
              ["all", `Tudo · ${tasks.length}`],
            ] as const).map(([value, label]) => (
              <button
                key={value}
                type="button"
                onClick={() => setOriginFilter(value)}
                aria-pressed={originFilter === value}
                className={cn(
                  "rounded-md px-2 py-1 text-[10px] transition-colors",
                  originFilter === value
                    ? "workspace-tab-active"
                    : "text-zinc-500 hover:text-zinc-200",
                )}
              >
                {label}
              </button>
            ))}
          </div>
        ) : null}
        {view !== "queue" ? (
          <>
            <FilterSearch
              value={query}
              onChange={setQuery}
              placeholder="Buscar por título, descrição, tag ou agente…"
              ariaLabel="Buscar tarefas"
            />
            <FilterSelect
              value={agentFilter}
              onChange={setAgentFilter}
              ariaLabel="Filtrar por agente"
            >
              <option value="">todos os agentes</option>
              {boardAgents.map((agent) => (
                <option key={agent} value={agent}>
                  @{agent}
                </option>
              ))}
            </FilterSelect>
            <FilterSelect
              value={priorityFilter}
              onChange={setPriorityFilter}
              ariaLabel="Filtrar por prioridade"
            >
              <option value="">todas as prioridades</option>
              {[4, 3, 2, 1].map((priority) => (
                <option key={priority} value={priority}>
                  {priority} — {PRIORITY_LABEL[priority]}
                </option>
              ))}
            </FilterSelect>
            <Badge variant="default">
              {filteredTasks.length} de {tasks.length}
            </Badge>
            <div className="relative">
              <Button
                variant="outline"
                size="sm"
                className="h-8"
                onClick={() => setVisibilityOpen((current) => !current)}
                title="Escolher quanto tempo tarefas concluídas permanecem visíveis"
                aria-expanded={visibilityOpen}
              >
                <IconLabel icon={<SlidersHorizontal size={12} />}>
                  Visibilidade
                </IconLabel>
              </Button>
              {visibilityOpen ? (
                <div className="absolute right-0 top-9 z-40 w-72 space-y-3 rounded-xl border border-zinc-700 bg-zinc-950 p-3 shadow-2xl">
                  <div>
                    <p className="text-[11px] font-semibold text-zinc-200">
                      Concluídas e canceladas
                    </p>
                    <p className="text-[10px] leading-snug text-zinc-500">
                      Apenas visual. Nada é apagado do workspace.
                    </p>
                  </div>
                  <label className="block space-y-1">
                    <span className="text-[10px] text-zinc-400">Período</span>
                    <select
                      aria-label="Período de retenção"
                      value={visibility.retention}
                      onChange={(event) =>
                        updateVisibility({
                          ...visibility,
                          retention: event.target.value as TaskVisibility["retention"],
                        })
                      }
                      className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2 text-[11px] text-zinc-200"
                    >
                      {RETENTION_OPTIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label} — {option.hint}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="block space-y-1">
                    <span className="text-[10px] text-zinc-400">
                      Máximo de cards (0 = sem limite)
                    </span>
                    <input
                      aria-label="Máximo de cards concluídos"
                      type="number"
                      min={0}
                      max={200}
                      value={visibility.maxItems}
                      onChange={(event) =>
                        updateVisibility({
                          ...visibility,
                          maxItems: Math.max(
                            0,
                            Math.min(200, Number(event.target.value) || 0),
                          ),
                        })
                      }
                      className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2 text-[11px] text-zinc-200"
                    />
                  </label>
                  <div className="flex items-center justify-between gap-2 border-t border-zinc-800/70 pt-2">
                    <button
                      type="button"
                      className="text-[10px] text-zinc-500 hover:text-zinc-200"
                      onClick={() =>
                        updateVisibility({ retention: "all", maxItems: 0 })
                      }
                    >
                      mostrar tudo
                    </button>
                    <button
                      type="button"
                      className="text-[10px] text-zinc-500 hover:text-zinc-200"
                      onClick={() => setVisibilityOpen(false)}
                    >
                      fechar
                    </button>
                  </div>
                </div>
              ) : null}
            </div>
            {query || agentFilter || priorityFilter ? (
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
          </>
        ) : (
          <span className="ml-auto text-[10px] text-zinc-500">
            {queue.length > 0
              ? `${queue.length} job(s) na fila`
              : "fila vazia"}
          </span>
        )}
      </div>

      {selected.size > 0 ? (
        <div className="flex flex-wrap items-center gap-2 rounded-xl border border-blue-500/25 bg-blue-500/[0.06] px-3 py-2">
          <Badge variant="blue">{selected.size} selecionada(s)</Badge>
          <span className="text-[11px] text-zinc-400">mover para:</span>
          {COLUMNS.map((column) => (
            <Button
              key={column.id}
              variant="outline"
              size="sm"
              className="h-7 text-[10px]"
              title={column.description}
              onClick={async () => {
                for (const task of tasks.filter((item) => selected.has(item.id))) {
                  await transitionTask(task, column.dropStatus);
                }
                setSelected(new Set());
              }}
            >
              {column.label}
            </Button>
          ))}
          <button
            type="button"
            className="ml-auto text-[10px] text-zinc-500 hover:text-zinc-200"
            onClick={() => setSelected(new Set())}
          >
            limpar seleção
          </button>
        </div>
      ) : null}

      {view === "board" ? (
      <div
        ref={boardRef}
        role="region"
        aria-label="Quadro Kanban"
        tabIndex={0}
        className="-mx-1 flex gap-3 overflow-x-auto px-1 pb-2 outline-none focus-visible:ring-2 focus-visible:ring-blue-500/50"
        onDragOver={(event) => {
          event.preventDefault();
          const element = boardRef.current;
          if (!element) return;
          const rect = element.getBoundingClientRect();
          if (event.clientX < rect.left + 72) element.scrollLeft -= 18;
          else if (event.clientX > rect.right - 72) element.scrollLeft += 18;
        }}
      >
        {COLUMNS.map((column) => {
          const columnTasks = filteredTasks.filter((task) =>
            column.states.includes(workflowMeta(task).state),
          );
          return (
            <Card
              key={column.id}
              data-column={column.id}
              className={cn(
                "flex min-h-[440px] w-[280px] shrink-0 flex-col transition-colors xl:w-auto xl:min-w-[220px] xl:flex-1",
                dragOverColumn === column.id &&
                  "border-blue-500/50 bg-blue-500/[0.04]",
              )}
              onDragEnter={() => setDragOverColumn(column.id)}
              onDragLeave={(event) => {
                if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
                  setDragOverColumn((current) =>
                    current === column.id ? null : current,
                  );
                }
              }}
              onDragOver={(event) => event.preventDefault()}
              onDrop={(event) => {
                const taskId =
                  event.dataTransfer.getData("text/task-id") ||
                  draggedRef.current;
                draggedRef.current = null;
                setDragOverColumn(null);
                const task = tasks.find((item) => item.id === taskId);
                if (!task) return;
                if (column.dropStatus === "BLOCKED") {
                  setBlockReason("");
                  setBlockingTask(task);
                } else {
                  void transitionTask(task, column.dropStatus);
                }
              }}
            >
              <CardHeader className="px-3.5 py-3">
                <CardTitle
                  className="flex items-center gap-2 text-[13.5px]"
                  title={column.description}
                >
                  <span className="h-1.5 w-1.5 rounded-full bg-zinc-600" />
                  {column.label}
                </CardTitle>
                <CountPill value={columnTasks.length} tone={column.tone} />
              </CardHeader>
              <CardContent className="flex-1 space-y-2 p-2.5">
                {columnTasks.length === 0 ? (
                  <div className="rounded-lg border border-dashed border-zinc-800/80 bg-zinc-950/30 px-3 py-10 text-center">
                    <GripVertical size={16} className="mx-auto text-zinc-600" />
                    <p className="mt-2 text-[12px] font-medium text-zinc-400">
                      Nenhuma tarefa nesta etapa
                    </p>
                    <p className="mt-0.5 text-[11px] text-zinc-500">
                      Arraste um card para mover.
                    </p>
                  </div>
                ) : (
                  columnTasks.map((task) => {
                    return (
                      <TaskCard
                        key={task.id}
                        task={task}
                        pinned={pinned[task.id] ?? 0}
                        daemonOnline={Boolean(daemon?.online)}
                        onOpen={() => setSelectedTask(task)}
                        onDispatch={() => void dispatch(task)}
                        onResume={() => void holdTask(task, false)}
                        onCancel={() => void transitionTask(task, "CANCELLED")}
                        onApprove={() => void approve(task)}
                        onLogs={() => setLogTask(task)}
                        onDragStart={(event) => {
                          draggedRef.current = task.id;
                          event.dataTransfer.setData("text/task-id", task.id);
                          event.dataTransfer.effectAllowed = "move";
                        }}
                        onDragEnd={() => {
                          // Keep the ref until the drop handler runs.
                        }}
                      />
                    );
                  })
                )}
              </CardContent>
            </Card>
          );
        })}
      </div>
      ) : view === "list" ? (
        <Card>
          <CardContent className="p-0">
            <div className="scroll-area max-h-[calc(100dvh-18rem)]">
              <table className="w-full text-left">
                <thead className="sticky top-0 z-10 border-b border-zinc-800/70 bg-zinc-950/95 text-[10px] uppercase tracking-wide text-zinc-500">
                  <tr>
                    <th className="w-8 px-3 py-2">
                      <input
                        type="checkbox"
                        checked={filteredTasks.length > 0 && selected.size === filteredTasks.length}
                        onChange={(event) =>
                          setSelected(
                            event.target.checked
                              ? new Set(filteredTasks.map((task) => task.id))
                              : new Set(),
                          )
                        }
                        className="accent-blue-500"
                      />
                    </th>
                    <th className="px-3 py-2">Tarefa</th>
                    <th className="px-3 py-2">Status</th>
                    <th className="px-3 py-2">Agente</th>
                    <th className="px-3 py-2">Último run</th>
                    <th className="px-3 py-2">Prioridade</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredTasks.map((task) => (
                    <tr
                      key={task.id}
                      className="cursor-pointer border-b border-zinc-800/50 text-[12px] hover:bg-zinc-900/40"
                      onClick={() => setSelectedTask(task)}
                    >
                      <td className="px-3 py-2" onClick={(event) => event.stopPropagation()}>
                        <input
                          type="checkbox"
                          checked={selected.has(task.id)}
                          onChange={(event) => {
                            setSelected((current) => {
                              const next = new Set(current);
                              if (event.target.checked) next.add(task.id);
                              else next.delete(task.id);
                              return next;
                            });
                          }}
                          className="accent-blue-500"
                        />
                      </td>
                      <td className="max-w-[360px] truncate px-3 py-2 font-medium text-zinc-200">
                        {task.title}
                      </td>
                      <td className="px-3 py-2">
                        <WorkflowStatusBadge task={task} />
                      </td>
                      <td className="px-3 py-2 font-mono text-[11px] text-zinc-400">
                        {task.assigned_agent ? `@${task.assigned_agent}` : "router"}
                      </td>
                      <td className="px-3 py-2">
                        {task.last_run_status ? (
                          <span
                            className={cn(
                              "font-mono text-[11px]",
                              task.last_run_status === "SUCCESS"
                                ? "text-emerald-300"
                                : task.last_run_status === "FAILED"
                                  ? "text-red-300"
                                  : "text-zinc-400",
                            )}
                          >
                            {task.last_run_status} · {relativeTime(task.last_run_at)}
                          </span>
                        ) : (
                          <span className="text-[11px] text-zinc-500">sem execução</span>
                        )}
                      </td>
                      <td className="px-3 py-2 text-zinc-400">
                        {PRIORITY_LABEL[task.priority] ?? task.priority}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </CardContent>
        </Card>
      ) : (
        <QueueController
          queue={queue}
          status={queueStatus}
          loading={loading}
          onRefresh={() => void load()}
          onTogglePause={(pause) => void toggleQueue(pause)}
          onConcurrency={(value) => void setConcurrency(value)}
          onJobAction={(id, action) => void jobAction(id, action)}
          onPriority={(id, priority) => void updateJobPriority(id, priority)}
        />
      )}

      <Modal
        open={!!blockingTask}
        onClose={() => setBlockingTask(null)}
        title="Mover para Atenção"
        description="Explique por que esta tarefa precisa de intervenção humana."
        size="md"
      >
        <div className="space-y-3">
          <label className="block">
            <span className="mb-1.5 block text-[10px] font-medium uppercase tracking-[0.12em] text-zinc-500">
              Motivo do bloqueio
            </span>
            <textarea
              value={blockReason}
              onChange={(event) => setBlockReason(event.target.value)}
              rows={3}
              autoFocus
              placeholder="Ex.: falta acesso ao Drive, precisa revisar o escopo..."
              className="w-full resize-y rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 py-2 text-[11px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
            />
          </label>
          <div className="flex justify-end gap-2">
            <Button variant="ghost" size="sm" onClick={() => setBlockingTask(null)}>
              Cancelar
            </Button>
            <Button
              size="sm"
              onClick={() => {
                if (!blockingTask) return;
                const reason = blockReason.trim() || "Bloqueada manualmente";
                setBlockingTask(null);
                void transitionTask(blockingTask, "BLOCKED", reason);
              }}
            >
              Bloquear tarefa
            </Button>
          </div>
        </div>
      </Modal>

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
    </PageBody>
  );
}
