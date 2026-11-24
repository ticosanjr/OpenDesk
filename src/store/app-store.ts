import { create } from "zustand";
import { apiGet, apiPost, websocketUrl } from "@/lib/api";
import type {
  BudgetStatus,
  DaemonStatus,
  DashboardStats,
  QueueJob,
  QueueStatus,
  Workspace,
  WsEvent,
} from "@/lib/types";
import type {
  AppStore,
  CreateWorkspaceInput,
  LiveRevisions,
} from "@/store/app-store-types";
import { reduceEvent } from "@/store/app-store-events";

export type { AppStore, CreateWorkspaceInput } from "@/store/app-store-types";

let socket: WebSocket | null = null;
let reconnectTimer: number | null = null;
let reconnectAttempts = 0;
let bootPromise: Promise<void> | null = null;
let workspaceRequestSeq = 0;
let liveRefreshTimer: number | null = null;
let visibilityBound = false;

const EMPTY_REVISIONS: LiveRevisions = {
  queue: 0,
  tasks: 0,
  runs: 0,
  artifacts: 0,
  attention: 0,
  schedules: 0,
  recipes: 0,
  memory: 0,
  governance: 0,
  studio: 0,
};

function attentionTotal(value: unknown): number {
  if (!value || typeof value !== "object") return 0;
  return [
    "requests",
    "missed_runs",
    "task_interactions",
    "hitl",
    "proposals",
    "awaiting_approval",
    "blocked",
    "failed_runs",
  ].reduce((total, key) => {
    const list = (value as Record<string, unknown>)[key];
    return total + (Array.isArray(list) ? list.length : 0);
  }, 0);
}

function validQueueStatus(value: unknown): value is QueueStatus {
  return Boolean(
    value &&
      typeof value === "object" &&
      !Array.isArray(value) &&
      Number.isFinite((value as QueueStatus).running),
  );
}

function scheduleLiveRefresh() {
  if (typeof window === "undefined") return;
  if (liveRefreshTimer !== null) window.clearTimeout(liveRefreshTimer);
  liveRefreshTimer = window.setTimeout(() => {
    liveRefreshTimer = null;
    void useAppStore.getState().refreshLiveState();
  }, 150);
}

export function handleEvent(event: WsEvent) {
  reduceEvent(event, {
    getState: useAppStore.getState,
    setState: useAppStore.setState,
    scheduleLiveRefresh,
  });
}

function bindVisibilityRefresh() {
  if (visibilityBound || typeof document === "undefined") return;
  visibilityBound = true;
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) void useAppStore.getState().refreshLiveState();
  });
}

export const useAppStore = create<AppStore>((set, get) => ({
  booted: false,
  loading: true,
  error: null,
  connected: false,
  daemon: null,
  workspaces: [],
  activeWorkspaceId: null,
  dashboard: null,
  budget: null,
  queue: [],
  queueStatus: null,
  attentionCount: 0,
  revisions: { ...EMPTY_REVISIONS },
  wsConnected: false,
  lastEventId: 0,
  lastFsEvent: null,
  lastTaskEvent: null,
  lastRunEvent: null,
  lastRunChunk: null,
  lastArtifactEvent: 0,
  lastArtifact: null,
  lastRecipeEvent: 0,
  lastProposalEvent: 0,
  lastPermissionEvent: null,
  lastScheduleEvent: null,
  lastChatEvent: null,
  lastGoogleEvent: null,
  lastEngineUpdate: null,
  budgetAlert: null,

  boot: async () => {
    if (bootPromise) return bootPromise;

    bootPromise = (async () => {
      bindVisibilityRefresh();
      set({ loading: true, error: null });
      try {
        await get().refresh();
        set({ booted: true });
        await connectSocket();
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : String(error),
          booted: false,
        });
      } finally {
        set({ loading: false });
      }
    })();

    try {
      await bootPromise;
    } finally {
      bootPromise = null;
    }
  },

  refresh: async () => {
    const requestId = ++workspaceRequestSeq;
    const previousWorkspaceId = get().activeWorkspaceId;
    const previousDashboard = get().dashboard;
    const [daemonResult, workspacesResult, budgetResult] = await Promise.allSettled([
      apiGet<DaemonStatus>("/api/daemon/status"),
      apiGet<Workspace[]>("/api/workspaces"),
      apiGet<BudgetStatus>("/api/budget/status"),
    ]);

    if (daemonResult.status === "fulfilled") {
      set({ daemon: daemonResult.value });
    }
    if (budgetResult.status === "fulfilled" && isValidBudget(budgetResult.value)) {
      set({ budget: budgetResult.value });
    }
    if (workspacesResult.status === "rejected") {
      throw workspacesResult.reason;
    }

    const workspaces = workspacesResult.value;
    let activeWorkspaceId = previousWorkspaceId;
    if (!activeWorkspaceId && workspaces.length > 0) {
      try {
        activeWorkspaceId = await apiGet<string>(
          "/api/settings/active_workspace_id",
        );
      } catch {
        activeWorkspaceId = workspaces[0]?.id ?? null;
      }
    }

    if (
      activeWorkspaceId &&
      !workspaces.some((workspace) => workspace.id === activeWorkspaceId)
    ) {
      activeWorkspaceId = workspaces[0]?.id ?? null;
    }

    // Publish the workspace list before the dashboard. A slow or broken
    // dashboard must never hide an otherwise loaded workspace.
    set({
      daemon: get().daemon,
      workspaces,
      activeWorkspaceId,
      dashboard:
        previousWorkspaceId === activeWorkspaceId ? previousDashboard : null,
    });

    if (activeWorkspaceId) {
      const [dashboardResult, queueResult, statusResult, attentionResult] =
        await Promise.allSettled([
          apiGet<DashboardStats>(
            `/api/dashboard?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
          ),
          apiGet<QueueJob[]>(
            `/api/queue?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
          ),
          apiGet<QueueStatus>(
            `/api/queue/status?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
          ),
          apiGet<unknown>(
            `/api/attention?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
          ),
        ]);
      if (
        requestId !== workspaceRequestSeq ||
        get().activeWorkspaceId !== activeWorkspaceId
      ) {
        return;
      }
      if (dashboardResult.status === "fulfilled") {
        set({ dashboard: dashboardResult.value });
      }
      if (queueResult.status === "fulfilled" && Array.isArray(queueResult.value)) {
        set({ queue: queueResult.value });
      }
      if (statusResult.status === "fulfilled" && validQueueStatus(statusResult.value)) {
        set({ queueStatus: statusResult.value });
      }
      if (attentionResult.status === "fulfilled") {
        set({ attentionCount: attentionTotal(attentionResult.value) });
      }
    } else {
      set({ queue: [], queueStatus: null, attentionCount: 0 });
    }
  },

  refreshLiveState: async (workspaceId) => {
    const activeWorkspaceId = workspaceId ?? get().activeWorkspaceId;
    if (!activeWorkspaceId) {
      set({ queue: [], queueStatus: null });
      return;
    }
    const requestId = ++workspaceRequestSeq;
    const [dashboardResult, queueResult, statusResult, attentionResult] =
      await Promise.allSettled([
        apiGet<DashboardStats>(
          `/api/dashboard?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        ),
        apiGet<QueueJob[]>(
          `/api/queue?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        ),
        apiGet<QueueStatus>(
          `/api/queue/status?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        ),
        apiGet<unknown>(
          `/api/attention?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        ),
      ]);
    if (
      requestId !== workspaceRequestSeq ||
      get().activeWorkspaceId !== activeWorkspaceId
    ) {
      return;
    }
    if (dashboardResult.status === "fulfilled") {
      set({ dashboard: dashboardResult.value });
    }
    if (queueResult.status === "fulfilled" && Array.isArray(queueResult.value)) {
      set({ queue: queueResult.value });
    }
    if (statusResult.status === "fulfilled" && validQueueStatus(statusResult.value)) {
      set({ queueStatus: statusResult.value });
    }
    if (attentionResult.status === "fulfilled") {
      set({ attentionCount: attentionTotal(attentionResult.value) });
    }
  },

  setAttentionCount: (count) => set({ attentionCount: Math.max(0, count) }),

  selectWorkspace: async (id: string) => {
    const requestId = ++workspaceRequestSeq;
    const workspace = await apiPost<Workspace>(`/api/workspaces/${id}/activate`);
    set((state) => ({
      activeWorkspaceId: workspace.id,
      workspaces: state.workspaces.map((item) =>
        item.id === workspace.id ? workspace : item,
      ),
      dashboard: null,
      queue: [],
      queueStatus: null,
      attentionCount: 0,
    }));
    const [dashboard, queue, status] = await Promise.all([
      apiGet<DashboardStats>(
        `/api/dashboard?workspace_id=${encodeURIComponent(workspace.id)}`,
      ),
      apiGet<QueueJob[]>(
        `/api/queue?workspace_id=${encodeURIComponent(workspace.id)}`,
      ).catch(() => [] as QueueJob[]),
      apiGet<QueueStatus>(
        `/api/queue/status?workspace_id=${encodeURIComponent(workspace.id)}`,
      ).catch(() => null),
    ]);
    if (
      requestId !== workspaceRequestSeq ||
      get().activeWorkspaceId !== workspace.id
    ) {
      return;
    }
    set({
      dashboard,
      queue: Array.isArray(queue) ? queue : [],
      queueStatus: validQueueStatus(status) ? status : null,
    });
  },

  createWorkspace: async (input: CreateWorkspaceInput) => {
    workspaceRequestSeq += 1;
    const workspace = await apiPost<Workspace>("/api/workspaces", input);
    set((state) => ({
      workspaces: [workspace, ...state.workspaces],
      activeWorkspaceId: workspace.id,
      dashboard: null,
      queue: [],
      queueStatus: null,
      attentionCount: 0,
    }));
    return workspace;
  },

  restartDaemon: async () => {
    await apiPost<{ ok: boolean }>("/api/daemon/restart");
    set((state) => ({
      daemon: state.daemon
        ? { ...state.daemon, state: "restarting", online: false }
        : null,
    }));
  },
}));

/** Runtime guard: older or partial API mocks must not poison the shell. */
function isValidBudget(value: BudgetStatus | null | undefined): value is BudgetStatus {
  return Boolean(
    value &&
      typeof value === "object" &&
      Number.isFinite(value.daily_limit) &&
      Number.isFinite(value.daily_spent),
  );
}

async function connectSocket() {
  if (
    socket &&
    (socket.readyState === WebSocket.OPEN ||
      socket.readyState === WebSocket.CONNECTING)
  ) {
    return;
  }

  const url = await websocketUrl();
  const wasReconnect = reconnectAttempts > 0;
  socket = new WebSocket(url);

  socket.onopen = () => {
    reconnectAttempts = 0;
    bindVisibilityRefresh();
    useAppStore.setState({ connected: true, wsConnected: true });
    if (wasReconnect) void useAppStore.getState().refreshLiveState();
  };

  socket.onmessage = (message) => {
    try {
      const event = JSON.parse(message.data as string) as WsEvent;
      handleEvent(event);
    } catch (error) {
      console.warn("OpenDesk: invalid websocket event", error);
    }
  };

  socket.onerror = () => {
    socket?.close();
  };

  socket.onclose = () => {
    useAppStore.setState({ connected: false, wsConnected: false });
    scheduleReconnect();
  };
}

function scheduleReconnect() {
  if (reconnectTimer !== null) return;
  const delay = Math.min(1000 * 2 ** reconnectAttempts, 15_000);
  reconnectAttempts += 1;
  reconnectTimer = window.setTimeout(() => {
    reconnectTimer = null;
    void connectSocket();
  }, delay);
}
