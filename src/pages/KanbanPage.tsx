import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";

import {
  CheckCircle2,
  Columns3,
  LayoutList,
  ListChecks,
  Plus,
  SlidersHorizontal,
  X,
} from "lucide-react";
import { PageBody, PageHeader } from "@/components/layout/PageHeader";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Modal } from "@/components/ui/modal";
import { IconLabel } from "@/components/ui/icon-label";
import { KanbanBoard } from "@/components/kanban/KanbanBoard";
import { KanbanListView } from "@/components/kanban/KanbanListView";
import { KANBAN_COLUMNS } from "@/components/kanban/kanban-columns";
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
import { useDismissable } from "@/lib/use-dismissable";
import { useKanbanViews } from "@/lib/kanban-views";
import { EmptyState } from "@/components/ui/empty-state";
import { FilterSearch } from "@/components/ui/filter-bar";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet, apiPatch, apiPost, apiPut } from "@/lib/api";
import type {
  DispatchDecision,
  QueueJob,
  Artifact,
  QueueStatus,
  Task,
  TaskInteraction,
} from "@/lib/types";
import type { TaskPermissionRequest } from "@/components/kanban/task-attention-modal";
import { cn } from "@/lib/utils";
import {
  applyRetention,
  DEFAULT_VISIBILITY,
  loadVisibility,
  RETENTION_OPTIONS,
  saveVisibility,
  type TaskVisibility,
} from "@/lib/task-visibility";

export function KanbanPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const queue = useAppStore((state) => state.queue);
  const daemon = useAppStore((state) => state.daemon);
  const lastTaskEvent = useAppStore((state) => state.lastTaskEvent);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const lastPermissionEvent = useAppStore((state) => state.lastPermissionEvent);
  const setWorkspaceDialogOpen = useUiStore(
    (state) => state.setWorkspaceDialogOpen,
  );
  const taskDialogOpen = useUiStore((state) => state.taskDialogOpen);
  const setTaskDialogOpen = useUiStore((state) => state.setTaskDialogOpen);

  const [tasks, setTasks] = useState<Task[]>([]);
  const [attention, setAttention] = useState<{
    task_interactions: TaskInteraction[];
    requests: TaskPermissionRequest[];
  }>({ task_interactions: [], requests: [] });
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
  const [attentionOnly, setAttentionOnly] = useState(false);
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [viewName, setViewName] = useState("");
  const { views: savedViews, save: saveView, remove: removeView } = useKanbanViews();
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
  const filtersRef = useRef<HTMLDivElement | null>(null);
  const visibilityRef = useRef<HTMLDivElement | null>(null);
  const [searchParams, setSearchParams] = useSearchParams();

  const workspace = workspaces.find((item) => item.id === activeWorkspaceId);
  const workspaceQuery = useMemo(
    () => (activeWorkspaceId ? `?workspace_id=${encodeURIComponent(activeWorkspaceId)}` : ""),
    [activeWorkspaceId],
  );

  const attentionByTask = useMemo(() => {
    const map = new Map<
      string,
      { interactions: TaskInteraction[]; permissions: TaskPermissionRequest[] }
    >();
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
  }, [attention]);

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
    return applyRetention(filtered, visibility);
  }, [
    agentFilter,
    attentionByTask,
    attentionOnly,
    originFilter,
    priorityFilter,
    query,
    tasks,
    visibility,
  ]);

  const originCounts = useMemo(() => {
    const manual = tasks.filter((task) => (task.origin ?? "MANUAL") === "MANUAL").length;
    return { manual, automations: tasks.length - manual };
  }, [tasks]);


  const advancedFilterCount =
    (agentFilter ? 1 : 0) +
    (priorityFilter ? 1 : 0) +
    (attentionOnly ? 1 : 0);

  const boardAgents = useMemo(
    () =>
      Array.from(
        new Set(tasks.map((task) => task.assigned_agent).filter(Boolean)),
      ).sort() as string[],
    [tasks],
  );


  useDismissable(filtersRef, filtersOpen, () => setFiltersOpen(false));
  useDismissable(visibilityRef, visibilityOpen, () => setVisibilityOpen(false));

  function applySavedView(view: (typeof savedViews)[number]) {
    setOriginFilter(view.origin);
    setQuery(view.query);
    setAgentFilter(view.agent);
    setPriorityFilter(view.priority);
    setAttentionOnly(view.attentionOnly);
    setFiltersOpen(false);
  }

  function saveCurrentView() {
    const name = viewName.trim();
    if (!name) return;
    saveView({
      name,
      origin: originFilter,
      query,
      agent: agentFilter,
      priority: priorityFilter,
      attentionOnly,
    });
    setViewName("");
  }

  const load = useCallback(async () => {
    if (!activeWorkspaceId) {
      setTasks([]);
      setQueueStatus(null);
      setAttention({ task_interactions: [], requests: [] });
      return;
    }
    setLoading(true);
    try {
      const [taskList, jobs, status, pinnedArtifacts, taskAttention] =
        await Promise.all([
          apiGet<Task[]>(`/api/tasks${workspaceQuery}`),
          apiGet<QueueJob[]>(`/api/queue${workspaceQuery}`),
          apiGet<QueueStatus>(`/api/queue/status${workspaceQuery}`),
          apiGet<Artifact[]>(`/api/artifacts/pinned${workspaceQuery}`).catch(() => []),
          apiGet<{
            task_interactions: TaskInteraction[];
            requests: TaskPermissionRequest[];
          }>(`/api/attention${workspaceQuery}`).catch(() => ({
            task_interactions: [],
            requests: [],
          })),
        ]);
      setTasks(taskList);
      setAttention(taskAttention);
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
  }, [
    load,
    lastTaskEvent?.task_id,
    lastRunEvent?.run_id,
    lastRunEvent?.status,
    lastPermissionEvent?.request_id,
  ]);

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
        <p className="rounded-lg border border-red-500/20 bg-red-500/10 px-3 py-2 text-[13px] text-red-300">
          {error}
        </p>
      ) : null}
      {notice ? (
        <p className="flex items-center gap-2 rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-[13px] text-emerald-300">
          <CheckCircle2 size={13} />
          {notice}
          <button
            type="button"
            className="ml-auto text-[12px] text-emerald-300/70 hover:text-emerald-200"
            onClick={() => setNotice(null)}
          >
            fechar
          </button>
        </p>
      ) : null}

      {decision ? (
        <div className="flex flex-wrap items-center gap-2 rounded-xl border border-blue-500/20 bg-blue-500/[0.06] px-4 py-3 text-[13px] text-blue-200">
          <Badge variant="blue">{decision.phase}</Badge>
          <span className="font-medium">
            {decision.selected_agent} · {decision.selected_model} · {decision.effort}
          </span>
          <span className="text-blue-300/70">{decision.rationale}</span>
          <button
            type="button"
            className="ml-auto text-[12px] text-blue-300/60 hover:text-blue-200"
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
                "flex items-center gap-1 rounded-md px-2 py-1 text-[12px] transition-colors",
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
                  "rounded-md px-2 py-1 text-[12px] transition-colors",
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
            <div ref={filtersRef} className="relative">
              <Button
                variant="outline"
                size="sm"
                className="h-8"
                onClick={() => setFiltersOpen((current) => !current)}
                aria-expanded={filtersOpen}
                title="Filtrar por agente, prioridade ou atenção"
              >
                <IconLabel icon={<SlidersHorizontal size={13} />}>
                  Filtros{advancedFilterCount > 0 ? ` · ${advancedFilterCount}` : ""}
                </IconLabel>
              </Button>
              {filtersOpen ? (
                <div className="absolute right-0 top-9 z-40 max-h-[min(70dvh,520px)] w-[320px] space-y-3 overflow-y-auto rounded-xl border border-zinc-700 bg-zinc-950 p-3 shadow-2xl">
                  <div className="flex items-center justify-between gap-2">
                    <p className="text-[13.5px] font-medium text-zinc-200">Filtros</p>
                    <button
                      type="button"
                      className="text-[13px] text-zinc-500 hover:text-zinc-200 disabled:opacity-40"
                      disabled={advancedFilterCount === 0}
                      onClick={() => {
                        setAgentFilter("");
                        setPriorityFilter("");
                        setAttentionOnly(false);
                      }}
                    >
                      limpar
                    </button>
                  </div>
                  <label className="block space-y-1">
                    <span className="text-[13px] text-zinc-400">Agente</span>
                    <select
                      value={agentFilter}
                      onChange={(event) => setAgentFilter(event.target.value)}
                      className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2 text-[13.5px] text-zinc-200"
                      aria-label="Filtrar por agente"
                    >
                      <option value="">todos os agentes</option>
                      {boardAgents.map((agent) => (
                        <option key={agent} value={agent}>
                          @{agent}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="block space-y-1">
                    <span className="text-[13px] text-zinc-400">Prioridade</span>
                    <select
                      value={priorityFilter}
                      onChange={(event) => setPriorityFilter(event.target.value)}
                      className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2 text-[13.5px] text-zinc-200"
                      aria-label="Filtrar por prioridade"
                    >
                      <option value="">todas as prioridades</option>
                      {[4, 3, 2, 1].map((priority) => (
                        <option key={priority} value={priority}>
                          {priority} — {PRIORITY_LABEL[priority]}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="flex items-center justify-between gap-2 rounded-lg border border-zinc-800/70 bg-zinc-900/30 px-2.5 py-2">
                    <span className="text-[13.5px] text-zinc-300">
                      Somente atenção
                    </span>
                    <input
                      type="checkbox"
                      checked={attentionOnly}
                      onChange={(event) => setAttentionOnly(event.target.checked)}
                      className="h-4 w-4 accent-blue-500"
                    />
                  </label>
                  <p className="text-[13px] leading-snug text-zinc-500">
                    Aprovação, pergunta, permissão ou falha. Bom para revisar o
                    que está parado.
                  </p>
                  <div className="space-y-2 border-t border-zinc-800/70 pt-3">
                    <div className="flex items-center justify-between gap-2">
                      <p className="text-[13px] font-medium uppercase tracking-[0.08em] text-zinc-400">
                        Visões salvas
                      </p>
                      <span className="font-mono text-[13px] text-zinc-500">
                        {savedViews.length}
                      </span>
                    </div>
                    {savedViews.length === 0 ? (
                      <p className="text-[13px] leading-snug text-zinc-500">
                        Salve a combinação atual para reaplicar depois.
                      </p>
                    ) : (
                      <div className="space-y-1">
                        {savedViews.map((view) => (
                          <div
                            key={view.id}
                            className="flex items-center gap-1 rounded-md border border-zinc-800/70 bg-zinc-900/30"
                          >
                            <button
                              type="button"
                              onClick={() => applySavedView(view)}
                              className="min-w-0 flex-1 truncate px-2 py-1.5 text-left text-[13.5px] text-zinc-300 hover:text-zinc-100"
                            >
                              {view.name}
                            </button>
                            <button
                              type="button"
                              aria-label={`Remover visão ${view.name}`}
                              title="Remover visão"
                              onClick={() => removeView(view.id)}
                              className="rounded p-1.5 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
                            >
                              <X size={12} />
                            </button>
                          </div>
                        ))}
                      </div>
                    )}
                    <div className="flex gap-1.5">
                      <input
                        value={viewName}
                        onChange={(event) => setViewName(event.target.value)}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") {
                            event.preventDefault();
                            saveCurrentView();
                          }
                        }}
                        placeholder="Nome da visão"
                        aria-label="Nome da visão"
                        className="h-8 min-w-0 flex-1 rounded-md border border-zinc-800 bg-zinc-900/60 px-2 text-[13.5px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
                      />
                      <Button
                        size="xs"
                        onClick={saveCurrentView}
                        disabled={!viewName.trim()}
                      >
                        Salvar
                      </Button>
                    </div>
                  </div>
                </div>
              ) : null}
            </div>
            <Badge variant="default">
              {filteredTasks.length} de {tasks.length}
            </Badge>
            <div ref={visibilityRef} className="relative">
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
                    <p className="text-[13px] font-semibold text-zinc-200">
                      Concluídas e canceladas
                    </p>
                    <p className="text-[12px] leading-snug text-zinc-500">
                      Apenas visual. Nada é apagado do workspace.
                    </p>
                  </div>
                  <label className="block space-y-1">
                    <span className="text-[12px] text-zinc-400">Período</span>
                    <select
                      aria-label="Período de retenção"
                      value={visibility.retention}
                      onChange={(event) =>
                        updateVisibility({
                          ...visibility,
                          retention: event.target.value as TaskVisibility["retention"],
                        })
                      }
                      className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2 text-[13px] text-zinc-200"
                    >
                      {RETENTION_OPTIONS.map((option) => (
                        <option key={option.value} value={option.value}>
                          {option.label} — {option.hint}
                        </option>
                      ))}
                    </select>
                  </label>
                  <label className="block space-y-1">
                    <span className="text-[12px] text-zinc-400">
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
                      className="h-8 w-full rounded-lg border border-zinc-800 bg-zinc-900/60 px-2 text-[13px] text-zinc-200"
                    />
                  </label>
                  <div className="flex items-center justify-between gap-2 border-t border-zinc-800/70 pt-2">
                    <button
                      type="button"
                      className="text-[12px] text-zinc-500 hover:text-zinc-200"
                      onClick={() =>
                        updateVisibility({ retention: "all", maxItems: 0 })
                      }
                    >
                      mostrar tudo
                    </button>
                    <button
                      type="button"
                      className="text-[12px] text-zinc-500 hover:text-zinc-200"
                      onClick={() => setVisibilityOpen(false)}
                    >
                      fechar
                    </button>
                  </div>
                </div>
              ) : null}
            </div>
          </>
        ) : (
          <span className="ml-auto text-[12px] text-zinc-500">
            {queue.length > 0
              ? `${queue.length} job(s) na fila`
              : "fila vazia"}
          </span>
        )}
      </div>

      {advancedFilterCount > 0 ? (
        <div className="flex flex-wrap items-center gap-1.5 px-1">
          <span className="text-[13px] text-zinc-500">Filtros ativos:</span>
          {agentFilter ? (
            <button
              type="button"
              onClick={() => setAgentFilter("")}
              aria-label={`Remover filtro do agente @${agentFilter}`}
              className="inline-flex items-center gap-1 rounded-full border border-zinc-700/70 bg-zinc-900/60 px-2 py-0.5 text-[13px] text-zinc-300 hover:border-zinc-600 hover:text-zinc-100"
            >
              @{agentFilter}
              <X size={11} />
            </button>
          ) : null}
          {priorityFilter ? (
            <button
              type="button"
              onClick={() => setPriorityFilter("")}
              aria-label={`Remover filtro de prioridade ${PRIORITY_LABEL[Number(priorityFilter)] ?? priorityFilter}`}
              className="inline-flex items-center gap-1 rounded-full border border-zinc-700/70 bg-zinc-900/60 px-2 py-0.5 text-[13px] text-zinc-300 hover:border-zinc-600 hover:text-zinc-100"
            >
              prioridade {PRIORITY_LABEL[Number(priorityFilter)] ?? priorityFilter}
              <X size={11} />
            </button>
          ) : null}
          {attentionOnly ? (
            <button
              type="button"
              onClick={() => setAttentionOnly(false)}
              aria-label="Remover filtro de atenção"
              className="inline-flex items-center gap-1 rounded-full border border-amber-500/30 bg-amber-500/10 px-2 py-0.5 text-[13px] text-amber-200 hover:border-amber-500/50"
            >
              atenção
              <X size={11} />
            </button>
          ) : null}
        </div>
      ) : null}

      {selected.size > 0 ? (
        <div className="flex flex-wrap items-center gap-2 rounded-xl border border-blue-500/25 bg-blue-500/[0.06] px-3 py-2">
          <Badge variant="blue">{selected.size} selecionada(s)</Badge>
          <span className="text-[13px] text-zinc-400">mover para:</span>
          {KANBAN_COLUMNS.map((column) => (
            <Button
              key={column.id}
              variant="outline"
              size="sm"
              className="h-7 text-[12px]"
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
            className="ml-auto text-[12px] text-zinc-500 hover:text-zinc-200"
            onClick={() => setSelected(new Set())}
          >
            limpar seleção
          </button>
        </div>
      ) : null}

      {view === "board" ? (
        <KanbanBoard
          tasks={filteredTasks}
          attentionByTask={attentionByTask}
          pinned={pinned}
          daemonOnline={Boolean(daemon?.online)}
          onOpenTask={setSelectedTask}
          onDispatch={(task) => void dispatch(task)}
          onResume={(task) => void holdTask(task, false)}
          onCancel={(task) => void transitionTask(task, "CANCELLED")}
          onApprove={(task) => void approve(task)}
          onLogs={setLogTask}
          onTransition={(task, status) => void transitionTask(task, status)}
          onBlock={(task) => {
            setBlockReason("");
            setBlockingTask(task);
          }}
          onAttentionResolved={() => void load()}
        />
      ) : view === "list" ? (
        <KanbanListView
          tasks={filteredTasks}
          selected={selected}
          onSelectAll={(checked) =>
            setSelected(
              checked ? new Set(filteredTasks.map((task) => task.id)) : new Set(),
            )
          }
          onToggleTask={(id, checked) =>
            setSelected((current) => {
              const next = new Set(current);
              if (checked) next.add(id);
              else next.delete(id);
              return next;
            })
          }
          onOpenTask={setSelectedTask}
        />
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
            <span className="mb-1.5 block text-[12px] font-medium uppercase tracking-[0.12em] text-zinc-500">
              Motivo do bloqueio
            </span>
            <textarea
              value={blockReason}
              onChange={(event) => setBlockReason(event.target.value)}
              rows={3}
              autoFocus
              placeholder="Ex.: falta acesso ao Drive, precisa revisar o escopo..."
              className="w-full resize-y rounded-lg border border-zinc-800 bg-zinc-900/50 px-2.5 py-2 text-[13px] text-zinc-200 outline-none placeholder:text-zinc-500 focus:border-blue-500/60"
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
