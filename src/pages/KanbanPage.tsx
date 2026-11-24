import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";

import { PageBody } from "@/components/layout/PageHeader";
import { KanbanBoard } from "@/components/kanban/KanbanBoard";
import { KanbanListView } from "@/components/kanban/KanbanListView";
import { isWorkQueueJob } from "@/lib/queue";
import { useDismissable } from "@/lib/use-dismissable";
import { useKanbanViews } from "@/lib/kanban-views";
import { useAppStore } from "@/store/app-store";
import { useUiStore } from "@/store/ui-store";
import { apiGet } from "@/lib/api";
import type {
  DispatchDecision,
  QueueJob,
  Artifact,
  QueueStatus,
  Task,
  TaskInteraction,
} from "@/lib/types";
import type { TaskPermissionRequest } from "@/components/kanban/task-attention-modal";
import {
  DEFAULT_VISIBILITY,
  loadVisibility,
  saveVisibility,
  type TaskVisibility,
} from "@/lib/task-visibility";
import { KanbanActiveFilters } from "./kanban/KanbanActiveFilters";
import { KanbanBlockModal } from "./kanban/KanbanBlockModal";
import { KanbanModals } from "./kanban/KanbanModals";
import { KanbanNoWorkspace } from "./kanban/KanbanNoWorkspace";
import { KanbanPageHeader } from "./kanban/KanbanPageHeader";
import { KanbanQueueView } from "./kanban/KanbanQueueView";
import { KanbanSelectionBar } from "./kanban/KanbanSelectionBar";
import { KanbanToolbar } from "./kanban/KanbanToolbar";
import { createKanbanActions } from "./kanban/kanban-actions";
import {
  buildAttentionByTask,
  collectBoardAgents,
  filterKanbanTasks,
} from "./kanban/kanban-derive";

export function KanbanPage() {
  const activeWorkspaceId = useAppStore((state) => state.activeWorkspaceId);
  const workspaces = useAppStore((state) => state.workspaces);
  const queue = useAppStore((state) => state.queue);
  const daemon = useAppStore((state) => state.daemon);
  const lastTaskEvent = useAppStore((state) => state.lastTaskEvent);
  const lastRunEvent = useAppStore((state) => state.lastRunEvent);
  const lastPermissionEvent = useAppStore((state) => state.lastPermissionEvent);
  const revisions = useAppStore((state) => state.revisions);
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
  const [executeTask, setExecuteTask] = useState<Task | null>(null);
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
  const [queueScope, setQueueScope] = useState<"work" | "automations" | "all">(
    "all",
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

  const workQueue = useMemo(() => queue.filter(isWorkQueueJob), [queue]);
  const automationQueue = useMemo(
    () => queue.filter((job) => !isWorkQueueJob(job)),
    [queue],
  );
  const workRunning = workQueue.filter((job) => job.status === "RUNNING").length;
  const displayedQueue =
    queueScope === "work"
      ? workQueue
      : queueScope === "automations"
        ? automationQueue
        : queue;

  const attentionByTask = useMemo(() => buildAttentionByTask(attention), [attention]);

  const filteredTasks = useMemo(
    () =>
      filterKanbanTasks({
        tasks,
        query,
        agentFilter,
        priorityFilter,
        attentionOnly,
        attentionByTask,
        visibility,
      }),
    [
      agentFilter,
      attentionByTask,
      attentionOnly,
      priorityFilter,
      query,
      tasks,
      visibility,
    ],
  );

  const advancedFilterCount =
    (agentFilter ? 1 : 0) +
    (priorityFilter ? 1 : 0) +
    (attentionOnly ? 1 : 0);

  const boardAgents = useMemo(() => collectBoardAgents(tasks), [tasks]);

  useDismissable(filtersRef, filtersOpen, () => setFiltersOpen(false));
  useDismissable(visibilityRef, visibilityOpen, () => setVisibilityOpen(false));

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
          apiGet<Task[]>(
            `/api/tasks${workspaceQuery}${workspaceQuery ? "&" : "?"}scope=work`,
          ),
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
      useAppStore.setState({ queue: jobs, queueStatus: status });
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
    revisions.tasks,
    revisions.runs,
    revisions.attention,
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

  const changeView = useCallback(
    (next: "board" | "list" | "queue") => {
      setView(next);
      const params = new URLSearchParams(searchParams);
      params.set("view", next);
      if (next !== "queue") params.delete("job");
      setSearchParams(params, { replace: true });
    },
    [searchParams, setSearchParams],
  );

  useEffect(() => {
    const requested = searchParams.get("view");
    if (requested === "queue" || requested === "list") {
      setView(requested);
      return;
    }
    setView("board");
  }, [searchParams]);

  // Deep links from the dashboard: /kanban?task=<id>&run=<id>
  useEffect(() => {
    const taskId = searchParams.get("task");
    if (!taskId) return;
    const runId = searchParams.get("run");
    const clearTaskParams = () => {
      const next = new URLSearchParams(searchParams);
      next.set("view", "board");
      next.delete("task");
      next.delete("run");
      setSearchParams(next, { replace: true });
    };
    const open = (task: Task) => {
      setView("board");
      setSelectedTask(task);
      if (runId) {
        setInitialRunId(runId);
        setLogTask(task);
      }
      clearTaskParams();
    };
    const task = tasks.find((item) => item.id === taskId);
    if (task) {
      open(task);
      return;
    }
    // Automation tasks are filtered out of the Kanban list, but links from
    // search, notifications and the attention card must still open them.
    let cancelled = false;
    void apiGet<Task>(`/api/tasks/${encodeURIComponent(taskId)}`)
      .then((fetched) => {
        if (!cancelled) open(fetched);
      })
      .catch(() => {
        if (!cancelled) clearTaskParams();
      });
    return () => {
      cancelled = true;
    };
  }, [tasks, searchParams, setSearchParams]);

  const {
    approve,
    transitionTask,
    holdTask,
    setWaiting,
    toggleQueue,
    setConcurrency,
    updateJobPriority,
    jobAction,
  } = createKanbanActions({
    load,
    setError,
    setTasks,
    setDecision,
    setSelectedTask,
  });

  if (!activeWorkspaceId) {
    return <KanbanNoWorkspace onCreateWorkspace={() => setWorkspaceDialogOpen(true)} />;
  }

  return (
    <PageBody>
      <KanbanPageHeader
        workspaceName={workspace?.name ?? "Workspace"}
        queueStatus={queueStatus}
        workRunning={workRunning}
        onCreateTask={() => setCreateOpen(true)}
        error={error}
        notice={notice}
        onDismissNotice={() => setNotice(null)}
        decision={decision}
        onDismissDecision={() => setDecision(null)}
      />

      <KanbanToolbar
        view={view}
        setView={changeView}
        query={query}
        setQuery={setQuery}
        filtersRef={filtersRef}
        filtersOpen={filtersOpen}
        setFiltersOpen={setFiltersOpen}
        advancedFilterCount={advancedFilterCount}
        agentFilter={agentFilter}
        setAgentFilter={setAgentFilter}
        priorityFilter={priorityFilter}
        setPriorityFilter={setPriorityFilter}
        attentionOnly={attentionOnly}
        setAttentionOnly={setAttentionOnly}
        boardAgents={boardAgents}
        filteredCount={filteredTasks.length}
        totalCount={tasks.length}
        visibilityRef={visibilityRef}
        visibilityOpen={visibilityOpen}
        setVisibilityOpen={setVisibilityOpen}
        visibility={visibility}
        onVisibilityChange={updateVisibility}
        savedViews={savedViews}
        removeView={removeView}
        viewName={viewName}
        setViewName={setViewName}
        saveView={saveView}
        queueLength={queue.length}
      />

      <KanbanActiveFilters
        advancedFilterCount={advancedFilterCount}
        agentFilter={agentFilter}
        priorityFilter={priorityFilter}
        attentionOnly={attentionOnly}
        onClearAgent={() => setAgentFilter("")}
        onClearPriority={() => setPriorityFilter("")}
        onClearAttention={() => setAttentionOnly(false)}
      />

      {selected.size > 0 ? (
        <KanbanSelectionBar
          selectedCount={selected.size}
          onMove={async (dropStatus) => {
            for (const task of tasks.filter((item) => selected.has(item.id))) {
              await transitionTask(task, dropStatus);
            }
            setSelected(new Set());
          }}
          onClear={() => setSelected(new Set())}
        />
      ) : null}

      {view === "board" ? (
        <KanbanBoard
          tasks={filteredTasks}
          attentionByTask={attentionByTask}
          pinned={pinned}
          daemonOnline={Boolean(daemon?.online)}
          onOpenTask={setSelectedTask}
          onDispatch={(task) => setExecuteTask(task)}
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
        <KanbanQueueView
          queueScope={queueScope}
          setQueueScope={setQueueScope}
          workQueueCount={workQueue.length}
          automationQueueCount={automationQueue.length}
          queueLength={queue.length}
          displayedQueue={displayedQueue}
          queueStatus={queueStatus}
          loading={loading}
          onRefresh={() => void load()}
          onTogglePause={(pause) => void toggleQueue(pause)}
          onConcurrency={(value) => void setConcurrency(value)}
          onJobAction={(id, action) => void jobAction(id, action)}
          onPriority={(id, priority) => void updateJobPriority(id, priority)}
        />
      )}

      <KanbanBlockModal
        open={!!blockingTask}
        onClose={() => setBlockingTask(null)}
        blockReason={blockReason}
        setBlockReason={setBlockReason}
        onConfirm={() => {
          if (!blockingTask) return;
          const reason = blockReason.trim() || "Bloqueada manualmente";
          setBlockingTask(null);
          void transitionTask(blockingTask, "BLOCKED", reason);
        }}
      />

      <KanbanModals
        createOpen={createOpen}
        setCreateOpen={setCreateOpen}
        workspaceId={activeWorkspaceId}
        load={load}
        selectedTask={selectedTask}
        setSelectedTask={setSelectedTask}
        editTask={editTask}
        setEditTask={setEditTask}
        executeTask={executeTask}
        setExecuteTask={setExecuteTask}
        forkSource={forkSource}
        setForkSource={setForkSource}
        swarmTask={swarmTask}
        setSwarmTask={setSwarmTask}
        logTask={logTask}
        setLogTask={setLogTask}
        initialRunId={initialRunId}
        setInitialRunId={setInitialRunId}
        lastRunEvent={lastRunEvent}
        setNotice={setNotice}
        setError={setError}
        approve={approve}
        transitionTask={transitionTask}
        setWaiting={setWaiting}
      />
    </PageBody>
  );
}
