import { create } from "zustand";
import { apiGet, apiPost, websocketUrl } from "@/lib/api";
import type {
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
  lastArtifactEvent: number;
  lastScheduleEvent: {
    schedule_id: string;
    workspace_id: string;
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
  lastArtifactEvent: 0,
  lastScheduleEvent: null,
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
    const [daemon, workspaces] = await Promise.all([
      apiGet<DaemonStatus>("/api/daemon/status"),
      apiGet<Workspace[]>("/api/workspaces"),
    ]);

    let activeWorkspaceId = get().activeWorkspaceId;
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

    let dashboard: DashboardStats | null = null;
    if (activeWorkspaceId) {
      dashboard = await apiGet<DashboardStats>(
        `/api/dashboard?workspace_id=${encodeURIComponent(activeWorkspaceId)}`,
      );
    }

    set({ daemon, workspaces, activeWorkspaceId, dashboard });
  },

  selectWorkspace: async (id: string) => {
    const workspace = await apiPost<Workspace>(`/api/workspaces/${id}/activate`);
    set((state) => ({
      activeWorkspaceId: workspace.id,
      workspaces: state.workspaces.map((item) =>
        item.id === workspace.id ? workspace : item,
      ),
    }));
    const dashboard = await apiGet<DashboardStats>(
      `/api/dashboard?workspace_id=${encodeURIComponent(workspace.id)}`,
    );
    set({ dashboard });
  },

  createWorkspace: async (input: CreateWorkspaceInput) => {
    const workspace = await apiPost<Workspace>("/api/workspaces", input);
    set((state) => ({
      workspaces: [workspace, ...state.workspaces],
      activeWorkspaceId: workspace.id,
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

function handleEvent(event: WsEvent) {
  const state = useAppStore.getState();
  if (event.event_id > state.lastEventId) {
    useAppStore.setState({ lastEventId: event.event_id });
  }

  switch (event.event_type) {
    case "DaemonStatusChanged":
      useAppStore.setState({ daemon: event.payload as DaemonStatus });
      break;
    case "QueueUpdated": {
      const payload = event.payload as { jobs?: QueueJob[] };
      useAppStore.setState({ queue: payload.jobs ?? [] });
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
        dashboard: current.dashboard
          ? {
              ...current.dashboard,
              artifacts_total: current.dashboard.artifacts_total + 1,
            }
          : current.dashboard,
      }));
      break;
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
