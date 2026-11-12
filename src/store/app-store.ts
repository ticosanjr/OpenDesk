import { create } from "zustand";
import { apiGet, apiPost, websocketUrl } from "@/lib/api";
import {
  markNotificationOnce,
  notificationEnabled,
  sendNativeNotification,
} from "@/lib/notification-policy";
import type {
  Artifact,
  DaemonStatus,
  DashboardStats,
  QueueJob,
  Workspace,
  WsEvent,
} from "@/lib/types";

export interface CreateWorkspaceInput {
  name: string;
  root_path: string;
  description?: string;
  color_hex?: string;
  default_router_policy?: string;
  timezone?: string;
}

interface AppStore {
  booted: boolean;
  loading: boolean;
  error: string | null;
  connected: boolean;
  daemon: DaemonStatus | null;
  workspaces: Workspace[];
  activeWorkspaceId: string | null;
  dashboard: DashboardStats | null;
  queue: QueueJob[];
  lastEventId: number;
  lastFsEvent: {
    entity: string;
    path: string;
    workspace_id: string;
  } | null;
  lastTaskEvent: {
    task_id: string;
    workspace_id: string;
    stage: string;
    phase_status?: string | null;
  } | null;
  lastRunEvent: {
    run_id: string;
    task_id?: string | null;
    status: string;
  } | null;
  lastRunChunk: {
    run_id: string;
    task_id?: string | null;
    chunk_type: string;
    seq: number;
  } | null;
  lastArtifactEvent: number;
  lastArtifact: Artifact | null;
  lastRecipeEvent: number;
  lastProposalEvent: number;
  lastPermissionEvent: {
    request_id: string;
    task_id?: string | null;
    workspace_id: string;
    action: string;
    resources: string[];
  } | null;
  lastScheduleEvent: {
    schedule_id: string;
    workspace_id: string;
  } | null;
  lastGoogleEvent: {
    workspace_id: string;
    connected: boolean;
    email?: string | null;
  } | null;
  lastEngineUpdate: {
    phase: string;
    message?: string | null;
    ok?: boolean | null;
  } | null;
  budgetAlert: {
    level: string;
    message: string;
    daily_spent: number;
    daily_limit: number;
  } | null;
  boot: () => Promise<void>;
  refresh: () => Promise<void>;
  selectWorkspace: (id: string) => Promise<void>;
  createWorkspace: (input: CreateWorkspaceInput) => Promise<Workspace>;
  restartDaemon: () => Promise<void>;
}

let socket: WebSocket | null = null;
let reconnectTimer: number | null = null;
let reconnectAttempts = 0;
let bootPromise: Promise<void> | null = null;

export const useAppStore = create<AppStore>((set, get) => ({
  booted: false,
  loading: true,
  error: null,
  connected: false,
  daemon: null,
  workspaces: [],
  activeWorkspaceId: null,
  dashboard: null,
  queue: [],
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
  lastGoogleEvent: null,
  lastEngineUpdate: null,
  budgetAlert: null,

  boot: async () => {
    if (bootPromise) return bootPromise;

    bootPromise = (async () => {
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
    const previousWorkspaceId = get().activeWorkspaceId;
    const previousDashboard = get().dashboard;
    const [daemonResult, workspacesResult] = await Promise.allSettled([
      apiGet<DaemonStatus>("/api/daemon/status"),
      apiGet<Workspace[]>("/api/workspaces"),
    ]);

    if (daemonResult.status === "fulfilled") {
      set({ daemon: daemonResult.value });
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
      const [dashboardResult, queueResult] = await Promise.allSettled([
        apiGet<DashboardStats>(
          `/api/dashboard?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        ),
        apiGet<QueueJob[]>(
          `/api/queue?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
        ),
      ]);
      if (dashboardResult.status === "fulfilled") {
        set({ dashboard: dashboardResult.value });
      }
      if (queueResult.status === "fulfilled") {
        set({ queue: queueResult.value });
      }
    } else {
      set({ queue: [] });
    }
  },

  selectWorkspace: async (id: string) => {
    const workspace = await apiPost<Workspace>(`/api/workspaces/${id}/activate`);
    set((state) => ({
      activeWorkspaceId: workspace.id,
      workspaces: state.workspaces.map((item) =>
        item.id === workspace.id ? workspace : item,
      ),
    }));
    const [dashboard, queue] = await Promise.all([
      apiGet<DashboardStats>(
        `/api/dashboard?workspace_id=${encodeURIComponent(workspace.id)}`,
      ),
      apiGet<QueueJob[]>(
        `/api/queue?workspace_id=${encodeURIComponent(workspace.id)}`,
      ).catch(() => [] as QueueJob[]),
    ]);
    set({ dashboard, queue });
  },

  createWorkspace: async (input: CreateWorkspaceInput) => {
    const workspace = await apiPost<Workspace>("/api/workspaces", input);
    set((state) => ({
      workspaces: [workspace, ...state.workspaces],
      activeWorkspaceId: workspace.id,
      queue: [],
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

async function connectSocket() {
  if (
    socket &&
    (socket.readyState === WebSocket.OPEN ||
      socket.readyState === WebSocket.CONNECTING)
  ) {
    return;
  }

  const url = await websocketUrl();
  socket = new WebSocket(url);

  socket.onopen = () => {
    reconnectAttempts = 0;
    useAppStore.setState({ connected: true });
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
    useAppStore.setState({ connected: false });
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

export function handleEvent(event: WsEvent) {
  const state = useAppStore.getState();
  if (event.event_id > state.lastEventId) {
    useAppStore.setState({ lastEventId: event.event_id });
  }

  switch (event.event_type) {
    case "DaemonStatusChanged":
      useAppStore.setState({ daemon: event.payload as DaemonStatus });
      break;
    case "QueueUpdated": {
      const payload = event.payload as {
        workspace_id?: string;
        jobs?: QueueJob[];
      };
      // Queue events are broadcast per workspace. Applying another workspace's
      // history made the sidebar badge show a number that did not belong to the
      // active board.
      if (
        !payload.workspace_id ||
        payload.workspace_id === useAppStore.getState().activeWorkspaceId
      ) {
        useAppStore.setState({ queue: payload.jobs ?? [] });
      }
      break;
    }
    case "FsSyncTriggered": {
      const payload = event.payload as {
        workspace_id?: string;
        entity?: string;
        path?: string;
      };
      useAppStore.setState({
        lastFsEvent: {
          entity: payload.entity ?? "file",
          path: payload.path ?? "",
          workspace_id: payload.workspace_id ?? "",
        },
      });
      break;
    }
    case "ResyncRequired":
      void state.refresh();
      break;
    case "TaskUpdated": {
      const payload = event.payload as {
        task_id?: string;
        workspace_id?: string;
        stage?: string;
        phase_status?: string | null;
      };
      if (payload.task_id && payload.workspace_id && payload.stage) {
        useAppStore.setState({
          lastTaskEvent: {
            task_id: payload.task_id,
            workspace_id: payload.workspace_id,
            stage: payload.stage,
            phase_status: payload.phase_status,
          },
        });
      }
      break;
    }
    case "RunStreamChunk": {
      const payload = event.payload as {
        run_id?: string;
        task_id?: string | null;
        chunk_type?: string;
      };
      if (payload.run_id) {
        useAppStore.setState({
          lastRunChunk: {
            run_id: payload.run_id,
            task_id: payload.task_id,
            chunk_type: payload.chunk_type ?? "event",
            seq: event.event_id,
          },
        });
      }
      break;
    }
    case "RecipeRunUpdated":
    case "HitlRequested":
    case "HitlResolved":
      useAppStore.setState((current) => ({ lastRecipeEvent: current.lastRecipeEvent + 1 }));
      break;
    case "ProposalCreated":
    case "ProposalResolved":
      useAppStore.setState((current) => ({ lastProposalEvent: current.lastProposalEvent + 1 }));
      break;
    case "PermissionRequested": {
      const payload = event.payload as {
        request_id?: string;
        task_id?: string | null;
        workspace_id?: string;
        action?: string;
        resources?: string[];
      };
      if (payload.request_id && payload.workspace_id) {
        useAppStore.setState({
          lastPermissionEvent: {
            request_id: payload.request_id,
            task_id: payload.task_id,
            workspace_id: payload.workspace_id,
            action: payload.action ?? "permission",
            resources: payload.resources ?? [],
          },
        });
        if (
          notificationEnabled("interactions") &&
          markNotificationOnce(`permission:${payload.request_id}`)
        ) {
          void sendNativeNotification(
            "Agente bloqueado esperando você",
            `${payload.action ?? "permissão"}: ${(payload.resources ?? []).join(", ").slice(0, 120)}`,
          );
        }
      }
      break;
    }
    case "RunFinished": {
      const payload = event.payload as {
        run_id?: string;
        task_id?: string | null;
        status?: string;
      };
      if (payload.run_id && payload.status) {
        useAppStore.setState({
          lastRunEvent: {
            run_id: payload.run_id,
            task_id: payload.task_id,
            status: payload.status,
          },
        });
      }
      break;
    }
    case "ArtifactCreated":
      useAppStore.setState((current) => ({
        lastArtifactEvent: current.lastArtifactEvent + 1,
        lastArtifact: (event.payload as { artifact?: Artifact }).artifact ?? null,
        dashboard: current.dashboard
          ? {
              ...current.dashboard,
              artifacts_total: current.dashboard.artifacts_total + 1,
            }
          : current.dashboard,
      }));
      break;
    case "GoogleConnectionUpdated": {
      const payload = event.payload as {
        workspace_id?: string;
        connected?: boolean;
        email?: string | null;
      };
      if (payload.workspace_id) {
        useAppStore.setState({
          lastGoogleEvent: {
            workspace_id: payload.workspace_id,
            connected: payload.connected ?? false,
            email: payload.email,
          },
        });
      }
      break;
    }
    case "EngineUpdateProgress": {
      const payload = event.payload as {
        phase?: string;
        message?: string | null;
        ok?: boolean | null;
      };
      useAppStore.setState({
        lastEngineUpdate: {
          phase: payload.phase ?? "running",
          message: payload.message,
          ok: payload.ok,
        },
      });
      break;
    }
    case "ScheduleUpdated": {
      const payload = event.payload as {
        schedule_id?: string;
        workspace_id?: string;
      };
      if (payload.schedule_id && payload.workspace_id) {
        useAppStore.setState({
          lastScheduleEvent: {
            schedule_id: payload.schedule_id,
            workspace_id: payload.workspace_id,
          },
        });
      }
      break;
    }
    case "BudgetAlert": {
      const payload = event.payload as {
        level?: string;
        message?: string;
        daily_spent?: number;
        daily_limit?: number;
      };
      useAppStore.setState({
        budgetAlert: {
          level: payload.level ?? "soft",
          message: payload.message ?? "orçamento próximo do limite",
          daily_spent: payload.daily_spent ?? 0,
          daily_limit: payload.daily_limit ?? 0,
        },
      });
      break;
    }
    default:
      break;
  }
}
