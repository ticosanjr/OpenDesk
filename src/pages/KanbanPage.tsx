import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
import {
  AlertTriangle,
  ArrowRightLeft,
  Ban,
  CheckCircle2,
  Circle,
  Columns3,
  Eye,
  FileText,
  GripVertical,
  GitBranch,
  LayoutList,
  ListChecks,
  Loader2,
  Pause,
  Pencil,
  Play,
  Plus,
  RotateCw,
  Search,
  SlidersHorizontal,
  Square,
  Star,
  XCircle,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { CountPill } from "@/components/ui/count-pill";
import { IconLabel } from "@/components/ui/icon-label";
import { Modal } from "@/components/ui/modal";
import { AgentCombobox } from "@/components/ui/agent-combobox";
import { CollapsibleSection } from "@/components/ui/collapsible-section";
import { ModelCombobox } from "@/components/ui/model-combobox";
import { SlashTextarea } from "@/components/ui/slash-textarea";
import { toast } from "@/store/toast-store";
import { RunLogView } from "@/components/runs/RunLogView";
import { QueueController } from "@/components/queue/QueueController";
import type { OutboundSink } from "@/components/integrations/OutboundSinksCard";
import { links } from "@/lib/links";
import { EntityLink } from "@/components/ui/entity-link";
import { EmptyState } from "@/components/ui/empty-state";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet, apiPatch, apiPost, apiPut } from "@/lib/api";
import type {
  DispatchDecision,
  QueueJob,
  Artifact,
  QueueStatus,
  Task,
  TaskDiagnostics,
  TaskRun,
} from "@/lib/types";
import { formatNumber, formatUsd, relativeTime } from "@/lib/format";
import { cn } from "@/lib/utils";
import { WorkingIndicator } from "@/components/ui/working-indicator";
import { Tooltip } from "@/components/ui/tooltip";
import {
  applyRetention,
  DEFAULT_VISIBILITY,
  loadVisibility,
  RETENTION_OPTIONS,
  saveVisibility,
  type TaskVisibility,
} from "@/lib/task-visibility";

const COLUMNS = [
  { id: "TODO", label: "To do", tone: "blue" as const, icon: Circle },
  { id: "IN_PROGRESS", label: "In progress", tone: "purple" as const, icon: Loader2 },
  { id: "IN_REVIEW", label: "In review", tone: "yellow" as const, icon: Eye },
  { id: "BLOCKED", label: "Blocked", tone: "red" as const, icon: Ban },
  { id: "DONE", label: "Done", tone: "green" as const, icon: CheckCircle2 },
  { id: "CANCELLED", label: "Cancelled", tone: "default" as const, icon: XCircle },
];

const STATUS_LABEL: Record<string, string> = Object.fromEntries(
  COLUMNS.map((column) => [column.id, column.label]),
);

const PRIORITY_LABEL: Record<number, string> = {
  1: "baixa",
  2: "normal",
  3: "alta",
  4: "crítica",
};

const PRIORITY_TONE: Record<number, "default" | "blue" | "yellow" | "red"> = {
  1: "default",
  2: "blue",
  3: "yellow",
  4: "red",
};

const EFFORT_TONE: Record<string, "default" | "blue" | "purple"> = {
  low: "default",
  medium: "blue",
  high: "purple",
};

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
  const [selected, setSelected] = useState<Set<string>>(new Set());
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
      if (needle) {
        const haystack = `${task.title} ${task.description} ${task.tags_json} ${task.assigned_agent ?? ""}`.toLowerCase();
        if (!haystack.includes(needle)) return false;
      }
      if (agentFilter && (task.assigned_agent ?? "") !== agentFilter) return false;
      if (priorityFilter && String(task.priority) !== priorityFilter) return false;
      return true;
    });
    return applyRetention(filtered, visibility);
  }, [agentFilter, priorityFilter, query, tasks, visibility]);

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

  async function transitionTask(task: Task, status: string) {
    if (!status || status === task.status) return;
    setError(null);
    setTasks((current) =>
      current.map((item) => (item.id === task.id ? { ...item, status } : item)),
    );
    try {
      await apiPost(`/api/tasks/${task.id}/transition`, { status });
      toast({
        tone: status === "CANCELLED" || status === "BLOCKED" ? "warning" : "success",
        title: `Tarefa movida para ${STATUS_LABEL[status] ?? status}`,
        description: task.title,
        href: links.task(task.id),
      });
      await load();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      await load();
    }
  }

  async function setWaiting(task: Task) {
    await transitionTask(task, "TODO");
  }

  async function forkTask(task: Task) {
    try {
      const forked = await apiPost<Task>(`/api/tasks/${task.id}/fork`, {});
      setSelectedTask(null);
      await load();
      toast({
        tone: "success",
        title: "Fork criado",
        description: forked.title,
        href: links.task(forked.id),
      });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
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
          <>
            <div className="flex h-8 min-w-[220px] flex-1 items-center gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-2.5">
              <Search size={13} className="shrink-0 text-zinc-600" />
              <input
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Buscar por título, descrição, tag ou agente…"
                className="h-full min-w-0 flex-1 bg-transparent text-[11px] outline-none placeholder:text-zinc-600"
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
          {["TODO", "IN_PROGRESS", "DONE", "BLOCKED", "CANCELLED"].map((status) => (
            <Button
              key={status}
              variant="outline"
              size="sm"
              className="h-7 text-[10px]"
              onClick={async () => {
                for (const task of tasks.filter((item) => selected.has(item.id))) {
                  await transitionTask(task, status);
                }
                setSelected(new Set());
              }}
            >
              {STATUS_LABEL[status] ?? status}
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
        className="-mx-1 flex gap-3 overflow-x-auto px-1 pb-2"
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
          const columnTasks = filteredTasks.filter((task) => task.status === column.id);
          return (
            <Card
              key={column.id}
              data-column={column.id}
              className={cn(
                "flex min-h-[440px] w-[280px] shrink-0 flex-col transition-colors",
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
                if (task) void transitionTask(task, column.id);
              }}
            >
              <CardHeader className="px-3 py-2.5">
                <CardTitle className="flex items-center gap-2 text-[12px]">
                  <span className="h-1.5 w-1.5 rounded-full bg-zinc-600" />
                  {column.label}
                </CardTitle>
                <CountPill value={columnTasks.length} tone={column.tone} />
              </CardHeader>
              <CardContent className="flex-1 space-y-1.5 p-2">
                {columnTasks.length === 0 ? (
                  <div className="rounded-lg border border-dashed border-zinc-800/80 bg-zinc-950/30 px-3 py-8 text-center">
                    <GripVertical size={15} className="mx-auto text-zinc-700" />
                    <p className="mt-2 text-[10px] text-zinc-600">Arraste uma tarefa</p>
                  </div>
                ) : (
                  columnTasks.map((task) => {
                    const meta =
                      COLUMNS.find((column) => column.id === task.status) ?? COLUMNS[0];
                    const StatusIcon = meta.icon;
                    const running =
                      task.status === "IN_PROGRESS" ||
                      task.phase_status === "queued" ||
                      task.phase_status === "running" ||
                      task.phase_status === "follow_up_queued";
                    return (
                      <div
                        key={task.id}
                        role="button"
                        tabIndex={0}
                        draggable
                        onDragStart={(event) => {
                          draggedRef.current = task.id;
                          event.dataTransfer.setData("text/task-id", task.id);
                          event.dataTransfer.effectAllowed = "move";
                        }}
                        onDragEnd={() => {
                          // Keep the ref until the drop handler runs.
                        }}
                        aria-label={`Abrir tarefa ${task.title}`}
                        className={cn(
                          "rounded-lg border bg-zinc-950/50 p-2 text-left outline-none transition-colors hover:border-zinc-700 focus-visible:ring-2 focus-visible:ring-blue-500/60",
                          task.status === "BLOCKED"
                            ? "border-red-500/40"
                            : task.status === "IN_PROGRESS"
                              ? "border-blue-500/40"
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
                              title="Arrastar tarefa"
                              onClick={(event) => event.stopPropagation()}
                              className="mt-0.5 cursor-grab text-zinc-600 active:cursor-grabbing"
                            >
                              <GripVertical size={12} />
                            </span>
                            <p className="line-clamp-2 text-[12px] font-semibold leading-snug text-zinc-100">
                              {task.title}
                            </p>
                          </div>
                          <Tooltip content={`Status: ${meta.label} (${task.status})`}>
                            <Badge
                              variant={meta.tone}
                              className="shrink-0 whitespace-nowrap"
                            >
                              <StatusIcon size={9} />
                              {meta.label}
                            </Badge>
                          </Tooltip>
                        </div>
                        <p className="mt-0.5 line-clamp-1 text-[10px] leading-snug text-zinc-500">
                          {task.description}
                        </p>
                        <div className="mt-1 flex flex-wrap items-center gap-1">
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
                          <Tooltip content={`Prioridade ${task.priority} de 4`}>
                            <Badge variant={PRIORITY_TONE[task.priority]}>
                              {PRIORITY_LABEL[task.priority] ?? task.priority}
                            </Badge>
                          </Tooltip>
                          <Tooltip content="Esforço solicitado ao roteador">
                            <Badge variant={EFFORT_TONE[task.effort_level] ?? "default"}>
                              {task.effort_level}
                            </Badge>
                          </Tooltip>
                          {pinned[task.id] ? (
                            <Badge variant="yellow" title="Entregáveis oficiais fixados">
                              <Star size={9} fill="currentColor" />
                              {pinned[task.id]}
                            </Badge>
                          ) : null}
                          {task.phase_status ? (
                            <Badge
                              variant={
                                task.phase_status.includes("failed")
                                  ? "red"
                                  : task.phase_status.includes("approval")
                                    ? "yellow"
                                    : task.phase_status === "waiting"
                                      ? "yellow"
                                      : "default"
                              }
                              title={task.blocked_reason ?? "Estado detalhado da execução"}
                            >
                              {task.phase_status}
                            </Badge>
                          ) : null}
                        </div>
                        <div className="mt-1.5 flex flex-wrap items-center gap-1.5 border-t border-zinc-800/60 pt-1.5">
                          {running ? <WorkingIndicator /> : null}
                          {task.last_run_status ? (
                            <span
                              className={cn(
                                "inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 font-mono text-[10px] font-semibold",
                                task.last_run_status === "SUCCESS"
                                  ? "border-emerald-500/25 bg-emerald-500/10 text-emerald-300"
                                  : task.last_run_status === "FAILED"
                                    ? "border-red-500/25 bg-red-500/10 text-red-300"
                                    : "border-zinc-700 bg-zinc-900 text-zinc-400",
                              )}
                              title={`Última execução: ${task.last_run_status} · ${relativeTime(task.last_run_at)}`}
                            >
                              {task.last_run_status === "SUCCESS" ? (
                                <CheckCircle2 size={10} />
                              ) : task.last_run_status === "FAILED" ? (
                                <XCircle size={10} />
                              ) : (
                                <Loader2 size={10} />
                              )}
                              {task.last_run_status}
                              <span className="text-zinc-600">
                                {relativeTime(task.last_run_at)}
                              </span>
                            </span>
                          ) : (
                            <span className="font-mono text-[10px] text-zinc-600">
                              sem execução
                            </span>
                          )}
                          <span className="ml-auto flex flex-wrap items-center gap-1">
                            {task.phase_status === "awaiting_approval" ? (
                              <Button
                                size="sm"
                                className="h-6 px-1.5 text-[10px]"
                                title="Aprovar o plano e iniciar o Build"
                                onClick={(event) => {
                                  event.stopPropagation();
                                  void approve(task);
                                }}
                              >
                                <CheckCircle2 size={11} />
                                Aprovar
                              </Button>
                            ) : running ? (
                              <Button
                                variant="destructive"
                                size="sm"
                                className="h-6 px-1.5 text-[10px]"
                                title="Cancelar a execução agora"
                                onClick={(event) => {
                                  event.stopPropagation();
                                  void transitionTask(task, "CANCELLED");
                                }}
                              >
                                <Square size={10} />
                                Cancelar
                              </Button>
                            ) : task.status !== "DONE" && task.status !== "CANCELLED" ? (
                              <Button
                                variant="outline"
                                size="sm"
                                className="h-6 px-1.5 text-[10px]"
                                title={
                                  daemon?.online
                                    ? "Criar uma nova tentativa de execução"
                                    : "OpenCode engine offline: inicie o motor para executar"
                                }
                                disabled={!daemon?.online}
                                onClick={(event) => {
                                  event.stopPropagation();
                                  void dispatch(task);
                                }}
                              >
                                <Play size={11} />
                                {task.status === "BLOCKED" ? "Tentar de novo" : "Executar"}
                              </Button>
                            ) : null}
                            <Button
                              variant="ghost"
                              size="sm"
                              className="h-6 px-1.5 text-[10px]"
                              title="Abrir a execução e o log completo"
                              onClick={(event) => {
                                event.stopPropagation();
                                setLogTask(task);
                              }}
                            >
                              <FileText size={11} />
                              Logs
                            </Button>
                          </span>
                        </div>
                      </div>
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
                        <Badge variant={COLUMNS.find((item) => item.id === task.status)?.tone ?? "default"}>
                          {STATUS_LABEL[task.status] ?? task.status}
                        </Badge>
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
                          <span className="text-[11px] text-zinc-600">sem execução</span>
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
        onFork={(task) => void forkTask(task)}
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
  const [model, setModel] = useState<string | null>(null);
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
      setModel(task.preferred_model ?? null);
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
      setModel(null);
      setEffort("medium");
      setEmail("");
      setExportToGdocs(false);
      setTags("");
    }
  }, [open, task]);

  if (!open) return null;
  const running =
    task?.status === "IN_PROGRESS" ||
    task?.phase_status === "queued" ||
    task?.phase_status === "running" ||
    task?.phase_status === "follow_up_queued";

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
        preferred_model: model,
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
          model_override: Boolean(model),
          clear_agent: !agent.trim(),
          clear_model: !model,
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

  const advancedSummary = [
    model ? `modelo: ${model}` : "roteador automático",
    `prioridade ${priority}`,
    agent ? `@${agent}` : null,
    tags.trim() ? "tags" : null,
    email.trim() ? "e-mail" : null,
    exportToGdocs ? "Google Docs" : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <Modal
      open={open}
      onClose={onClose}
      title={mode === "create" ? "Nova tarefa" : "Editar tarefa"}
      description={
        mode === "create"
          ? "Comece pelo título e prompt. Abra “Mais opções” para modo, agente, modelo, effort e notificações. O roteador decide o resto."
          : "Corrija agente, modelo, prompt e prioridade sem recriar o card."
      }
      className="max-w-2xl"
    >
      <div className="space-y-3">
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
      </div>
        <CollapsibleSection title="Mais opções" summary={advancedSummary}>
          <div className="grid gap-3 md:grid-cols-2">
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
        </CollapsibleSection>
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
  const [context, setContext] = useState<{
    session_id?: string | null;
    tokens_input: number;
    window: number;
    percent: number;
    messages: number;
  } | null>(null);
  const [compacting, setCompacting] = useState(false);
  const [sinks, setSinks] = useState<OutboundSink[]>([]);
  const [selectedSinks, setSelectedSinks] = useState<Set<string>>(new Set());
  const [savingSinks, setSavingSinks] = useState(false);
  const daemon = useAppStore((state) => state.daemon);

  useEffect(() => {
    if (!task) {
      setSinks([]);
      setSelectedSinks(new Set());
      return;
    }
    let cancelled = false;
    void Promise.all([
      apiGet<{ sinks: OutboundSink[] }>(
        `/api/io/outbound?workspace_id=${encodeURIComponent(task.workspace_id)}`,
      ),
      apiGet<{ bindings: Array<{ sink_id: string }> }>(
        `/api/tasks/${task.id}/sinks`,
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
  }, [task?.id]);

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

  const running =
    task?.status === "IN_PROGRESS" ||
    task?.phase_status === "queued" ||
    task?.phase_status === "running" ||
    task?.phase_status === "follow_up_queued";
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
          <div className="flex flex-wrap items-center gap-2">
            <Badge
              variant={
                task.status === "BLOCKED"
                  ? "red"
                  : task.status === "DONE"
                    ? "green"
                    : task.status === "IN_PROGRESS"
                      ? "blue"
                      : task.status === "CANCELLED"
                        ? "default"
                        : "yellow"
              }
            >
              {STATUS_LABEL[task.status] ?? task.status}
            </Badge>
            <Badge variant="default">prioridade {task.priority}</Badge>
            {task.last_run_status ? (
              <Badge variant={runVariant(task.last_run_status)}>
                último run: {task.last_run_status}
              </Badge>
            ) : (
              <Badge variant="default">sem execução</Badge>
            )}
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
              <Badge variant="blue" title={task.output_dir_path}>
                output
              </Badge>
            ) : null}
          </div>

          {task.blocked_reason ? (
            <p className="rounded-lg border border-red-500/25 bg-red-500/10 px-3 py-2 text-[11px] font-medium text-red-200">
              Motivo do bloqueio: {task.blocked_reason}
            </p>
          ) : null}

          {running ? (
            <p className="rounded-lg border border-blue-500/20 bg-blue-500/10 px-3 py-2 text-[10px] text-blue-200">
              Esta tarefa está em execução. Cancele no controlador da fila antes de
              editar agente, modelo ou prompt.
            </p>
          ) : null}

          {sinks.length > 0 ? (
            <div className="rounded-lg border border-zinc-800/80 bg-zinc-950/40 px-3 py-2">
              <p className="text-[10px] font-medium uppercase tracking-wide text-zinc-500">
                Entregas automáticas
              </p>
              <div className="mt-1.5 flex flex-wrap gap-x-4 gap-y-1">
                {sinks.map((sink) => (
                  <label key={sink.id} className="flex items-center gap-1.5 text-[10px] text-zinc-400">
                    <input
                      type="checkbox"
                      checked={selectedSinks.has(sink.id)}
                      onChange={(event) =>
                        setSelectedSinks((current) => {
                          const next = new Set(current);
                          if (event.target.checked) next.add(sink.id);
                          else next.delete(sink.id);
                          return next;
                        })
                      }
                      className="accent-blue-500"
                    />
                    {sink.name}
                  </label>
                ))}
                <Button
                  variant="outline"
                  size="sm"
                  className="ml-auto h-7"
                  disabled={savingSinks}
                  onClick={() => void saveSinks()}
                >
                  {savingSinks ? <Loader2 size={12} className="animate-spin" /> : null}
                  Salvar entregas
                </Button>
              </div>
            </div>
          ) : null}

          {context?.session_id ? (
            <div className="rounded-lg border border-zinc-800/80 bg-zinc-950/40 px-3 py-2">
              <div className="flex flex-wrap items-center gap-2">
                <span className="text-[10px] font-medium uppercase tracking-wide text-zinc-500">
                  contexto
                </span>
                <div className="h-1.5 min-w-32 flex-1 overflow-hidden rounded-full bg-zinc-800">
                  <div
                    className={cn(
                      "h-full rounded-full",
                      context.percent > 80
                        ? "bg-red-400"
                        : context.percent > 60
                          ? "bg-amber-400"
                          : "bg-blue-400",
                    )}
                    style={{ width: `${Math.min(100, context.percent)}%` }}
                  />
                </div>
                <span className="font-mono text-[10px] text-zinc-500">
                  {formatNumber(context.tokens_input)} / {formatNumber(context.window)} ·{" "}
                  {context.percent.toFixed(0)}% · {context.messages} msgs
                </span>
                <Button
                  variant="outline"
                  size="sm"
                  className="h-7"
                  disabled={compacting}
                  onClick={() => void compactContext()}
                >
                  {compacting ? <Loader2 size={12} className="animate-spin" /> : null}
                  Compactar contexto agora
                </Button>
              </div>
            </div>
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
            <Button
              variant="outline"
              size="sm"
              onClick={() => onFork(task)}
              title="Criar uma cópia independente desta tarefa e do estado atual"
            >
              <GitBranch size={13} />
              Fork
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => onSwarm(task)}
              title="Rodar a mesma tarefa em 2 modelos e comparar os entregáveis"
            >
              <Columns3 size={13} />
              A/B
            </Button>
            <Button variant="outline" size="sm" onClick={() => onEdit(task)}>
              <Pencil size={13} />
              Editar
            </Button>
            <Button variant="outline" size="sm" onClick={() => onLogs(task)}>
              <FileText size={13} />
              Execução
            </Button>
            {task.status !== "CANCELLED" && task.status !== "DONE" ? (
              <Button
                variant="destructive"
                size="sm"
                onClick={() => onStatus(task, "CANCELLED")}
                title="Cancelar imediatamente e interromper a execução"
              >
                <Square size={12} />
                Cancelar
              </Button>
            ) : null}
            {task.status !== "BLOCKED" && task.status !== "DONE" ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => onStatus(task, "BLOCKED")}
                title="Bloquear a tarefa e interromper a execução"
              >
                <Ban size={12} />
                Bloquear
              </Button>
            ) : null}
            {task.status === "IN_PROGRESS" || task.status === "TODO" ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => onWait(task)}
                title="Colocar em espera sem executar"
              >
                <Pause size={12} />
                Em espera
              </Button>
            ) : null}
            {task.status === "BLOCKED" || task.status === "CANCELLED" || task.status === "DONE" ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => onStatus(task, "TODO")}
                title="Reabrir a tarefa como To do"
              >
                <RotateCw size={12} />
                Reabrir
              </Button>
            ) : null}
            {task.status !== "DONE" ? (
              <Button
                variant="outline"
                size="sm"
                onClick={() => onStatus(task, "DONE")}
                title="Marcar como concluída sem executar"
              >
                <CheckCircle2 size={12} />
                Concluir
              </Button>
            ) : null}
            {task.phase_status === "awaiting_approval" ? (
              <Button size="sm" onClick={() => onApprove(task)}>
                <CheckCircle2 size={13} />
                Aprovar plano e iniciar Build
              </Button>
            ) : task.status !== "IN_PROGRESS" ? (
              <Button
                size="sm"
                onClick={() => onDispatch(task)}
                disabled={!daemon?.online}
                title={
                  daemon?.online
                    ? "Executar agora"
                    : "OpenCode engine offline: inicie o motor para executar"
                }
              >
                <Play size={13} />
                {task.status === "BLOCKED" ? "Nova tentativa" : "Executar"}
              </Button>
            ) : null}
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
      <aside className="relative flex h-dvh w-full max-w-5xl flex-col border-l border-zinc-800 bg-zinc-950 shadow-2xl">
        <div className="flex items-center justify-between border-b border-zinc-800/70 px-4 py-3">
          <div className="min-w-0">
            <p className="truncate text-[12px] font-medium text-zinc-200">
              {task.title}
            </p>
            <p className="font-mono text-[9px] text-zinc-600">
              {STATUS_LABEL[task.status] ?? task.status} · {task.phase_status ?? "sem fase"}
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
                    <p className="truncate text-[10px]">
                      tentativa {run.attempt_number ?? 1} · {run.phase ?? "run"}
                    </p>
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
          <div className="flex min-w-0 flex-1 flex-col overflow-hidden bg-zinc-950/60">
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


function SwarmModal({
  task,
  onClose,
  onCreated,
}: {
  task: Task | null;
  onClose: () => void;
  onCreated: () => Promise<void>;
}) {
  const [models, setModels] = useState([
    "opencode-go/deepseek-v4.1-flash",
    "opencode-go/glm-5.3",
  ]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<
    Array<{ id: string; title: string; model: string }>
  >([]);
  const [artifacts, setArtifacts] = useState<Record<string, Artifact[]>>({});
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);

  useEffect(() => {
    if (task) {
      setError(null);
      setCreated([]);
      setArtifacts({});
    }
  }, [task]);

  useEffect(() => {
    if (created.length === 0) return;
    void Promise.all(
      created.map(async (copy) => {
        const list = await apiGet<Artifact[]>(
          `/api/artifacts?task_id=${encodeURIComponent(copy.id)}`,
        ).catch(() => []);
        return [copy.id, list] as const;
      }),
    ).then((entries) => setArtifacts(Object.fromEntries(entries)));
  }, [created, lastRunEvent?.run_id]);

  async function run() {
    if (!task) return;
    setSaving(true);
    setError(null);
    try {
      const result = await apiPost<{
        tasks: Array<{ id: string; title: string; model: string }>;
      }>(`/api/tasks/${task.id}/swarm`, { models });
      setCreated(result.tasks);
      toast({
        tone: "success",
        title: `${result.tasks.length} cópias enfileiradas`,
        description: "Compare os entregáveis e marque o vencedor.",
      });
      await onCreated();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setSaving(false);
    }
  }

  async function chooseWinner(winnerId: string) {
    for (const copy of created) {
      await apiPost(`/api/tasks/${copy.id}/transition`, {
        status: copy.id === winnerId ? "DONE" : "CANCELLED",
      });
    }
    toast({ tone: "success", title: "Vencedor marcado como Done" });
    setCreated([]);
    onClose();
  }

  return (
    <Modal
      open={!!task}
      onClose={onClose}
      title="Enxame A/B"
      description="Duas cópias isoladas da mesma tarefa, com modelos diferentes. O original não muda."
      className="max-w-2xl"
    >
      <div className="space-y-4">
        {models.map((model, index) => (
          <label key={index}>
            <span className="mb-1 block text-[10px] font-medium uppercase tracking-wide text-zinc-500">
              Modelo {index + 1}
            </span>
            <ModelCombobox
              value={model}
              allowRouter={false}
              onChange={(value) => {
                if (!value) return;
                setModels((current) =>
                  current.map((item, itemIndex) =>
                    itemIndex === index ? value : item,
                  ),
                );
              }}
            />
          </label>
        ))}
        {error ? (
          <p className="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-2 text-[11px] text-red-200">
            {error}
          </p>
        ) : null}
        {created.length > 0 ? (
          <div className="grid gap-2 sm:grid-cols-2">
            {created.map((copy) => (
              <div
                key={copy.id}
                className="flex min-w-0 flex-col gap-2 rounded-lg border border-zinc-800 bg-zinc-950/40 px-3 py-2"
              >
                <div className="min-w-0">
                  <p className="truncate text-[12px] text-zinc-200">{copy.title}</p>
                  <p className="font-mono text-[10px] text-zinc-500">{copy.model}</p>
                </div>
                <div className="min-h-10 flex-1 space-y-1">
                  {(artifacts[copy.id] ?? []).length === 0 ? (
                    <p className="text-[10px] text-zinc-600">
                      aguardando entregáveis…
                    </p>
                  ) : (
                    artifacts[copy.id].slice(0, 6).map((artifact) => (
                      <button
                        key={artifact.id}
                        type="button"
                        onClick={() => {
                          window.location.hash = `#/artifacts?id=${encodeURIComponent(artifact.id)}`;
                        }}
                        className="flex w-full items-center gap-1.5 truncate rounded border border-zinc-800/70 px-1.5 py-1 text-left text-[10px] text-zinc-400 hover:text-zinc-100"
                        title={artifact.file_path}
                      >
                        <FileText size={10} className="shrink-0" />
                        <span className="truncate">{artifact.file_path}</span>
                      </button>
                    ))
                  )}
                </div>
                <div className="flex flex-wrap gap-1">
                  <Button
                    size="sm"
                    variant="outline"
                    className="h-7"
                    onClick={() => {
                      window.location.hash = `#${links.task(copy.id)}`;
                    }}
                  >
                    Abrir
                  </Button>
                  <Button
                    size="sm"
                    className="h-7"
                    onClick={() => void chooseWinner(copy.id)}
                  >
                    Vencedor
                  </Button>
                </div>
              </div>
            ))}
          </div>
        ) : null}
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={onClose}>
            Fechar
          </Button>
          <Button size="sm" onClick={() => void run()} disabled={saving || !task}>
            {saving ? <Loader2 size={13} className="animate-spin" /> : null}
            Rodar em paralelo
          </Button>
        </div>
      </div>
    </Modal>
  );
}
