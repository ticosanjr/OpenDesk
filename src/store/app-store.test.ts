import { beforeEach, describe, expect, it, vi, type Mock } from "vitest";

vi.mock("@/lib/api", () => ({
  apiGet: vi.fn(),
  apiPost: vi.fn(),
  websocketUrl: vi.fn(),
}));

import { apiGet, apiPost, websocketUrl } from "@/lib/api";
import { useAppStore } from "./app-store";

const daemon = {
  online: true,
  port: 3101,
  active_jobs: 0,
  state: "online",
  version: "2.2.5",
  mode: "dedicated",
};

beforeEach(() => {
  vi.clearAllMocks();
  useAppStore.setState({
    daemon: null,
    workspaces: [],
    activeWorkspaceId: null,
    dashboard: null,
    loading: false,
    error: null,
  });
});

describe("app store", () => {
  it("refreshes daemon and workspaces", async () => {
    (apiGet as Mock).mockImplementation(async (path: string) => {
      if (path.includes("daemon")) return daemon;
      return [];
    });

    await useAppStore.getState().refresh();
    expect(useAppStore.getState().daemon?.online).toBe(true);
    expect(useAppStore.getState().workspaces).toEqual([]);
  });

  it("falls back to the first workspace when the active one is missing", async () => {
    const workspace = {
      id: "ws-1",
      name: "WS",
      root_path: "/tmp/ws",
      color_hex: "#fff",
      default_router_policy: "COST_BENEFIT",
      timezone: "UTC",
      is_active: true,
      created_at: "",
      last_opened_at: "",
    };
    (apiGet as Mock).mockImplementation(async (path: string) => {
      if (path.includes("daemon")) return daemon;
      if (path.includes("workspaces")) return [workspace];
      if (path.includes("active_workspace_id")) throw new Error("missing");
      return { runs_total: 0 };
    });

    await useAppStore.getState().refresh();
    expect(useAppStore.getState().activeWorkspaceId).toBe("ws-1");
  });

  it("keeps workspaces visible when the dashboard fails", async () => {
    const workspace = {
      id: "ws-1",
      name: "WS",
      root_path: "/tmp/ws",
      color_hex: "#fff",
      default_router_policy: "COST_BENEFIT",
      timezone: "UTC",
      is_active: true,
      created_at: "",
      last_opened_at: "",
    };
    (apiGet as Mock).mockImplementation(async (path: string) => {
      if (path.includes("daemon")) return daemon;
      if (path.includes("workspaces")) return [workspace];
      if (path.includes("active_workspace_id")) return "ws-1";
      throw new Error("dashboard decode failed");
    });

    await expect(useAppStore.getState().refresh()).resolves.toBeUndefined();
    expect(useAppStore.getState().workspaces).toEqual([workspace]);
    expect(useAppStore.getState().activeWorkspaceId).toBe("ws-1");
    expect(useAppStore.getState().daemon?.online).toBe(true);
  });

  it("marks the daemon as restarting", async () => {
    (apiPost as Mock).mockResolvedValue({ ok: true });
    useAppStore.setState({ daemon });
    await useAppStore.getState().restartDaemon();
    expect(apiPost).toHaveBeenCalledWith("/api/daemon/restart");
    expect(useAppStore.getState().daemon).toMatchObject({
      online: false,
      state: "restarting",
    });
  });

  it("adds a created workspace to the front", async () => {
    const workspace = {
      id: "ws-2",
      name: "New",
      root_path: "/tmp/new",
      color_hex: "#fff",
      default_router_policy: "COST_BENEFIT",
      timezone: "UTC",
      is_active: true,
      created_at: "",
      last_opened_at: "",
    };
    (apiPost as Mock).mockResolvedValue(workspace);
    const created = await useAppStore.getState().createWorkspace({
      name: "New",
      root_path: "/tmp/new",
    });
    expect(created.id).toBe("ws-2");
    expect(useAppStore.getState().workspaces[0].id).toBe("ws-2");
    expect(useAppStore.getState().activeWorkspaceId).toBe("ws-2");
  });
});

describe("websocket event reducer", () => {
  it("applies daemon, queue and file events", async () => {
    const { handleEvent } = await import("./app-store");
    handleEvent({
      event_id: 5,
      event_type: "DaemonStatusChanged",
      payload: { ...daemon, online: false, state: "offline" },
    });
    expect(useAppStore.getState().daemon?.state).toBe("offline");
    expect(useAppStore.getState().lastEventId).toBe(5);

    handleEvent({
      event_id: 6,
      event_type: "QueueUpdated",
      payload: { jobs: [{ id: "job-1" }] },
    });
    expect(useAppStore.getState().queue).toHaveLength(1);

    handleEvent({
      event_id: 7,
      event_type: "FsSyncTriggered",
      payload: { workspace_id: "ws-1", entity: "agent", path: "/tmp/a.md" },
    });
    expect(useAppStore.getState().lastFsEvent).toMatchObject({
      entity: "agent",
      path: "/tmp/a.md",
      workspace_id: "ws-1",
    });
  });

  it("applies task, run, artifact, schedule and budget events", async () => {
    const { handleEvent } = await import("./app-store");
    useAppStore.setState({
      dashboard: {
        runs_total: 0,
        runs_success: 0,
        runs_failed: 0,
        runs_running: 0,
        tokens_input: 0,
        tokens_output: 0,
        tokens_cache_read: 0,
        tokens_cache_write: 0,
        estimated_cost_usd: 0,
        daily_cost_usd: 0,
        queue_running: 0,
        queue_waiting: 0,
        artifacts_total: 2,
      },
    });

    handleEvent({
      event_id: 10,
      event_type: "TaskUpdated",
      payload: { task_id: "t1", workspace_id: "ws", stage: "BUILD", phase_status: "running" },
    });
    expect(useAppStore.getState().lastTaskEvent?.task_id).toBe("t1");

    handleEvent({
      event_id: 11,
      event_type: "RunFinished",
      payload: { run_id: "r1", task_id: "t1", status: "SUCCESS" },
    });
    expect(useAppStore.getState().lastRunEvent?.status).toBe("SUCCESS");

    handleEvent({ event_id: 12, event_type: "ArtifactCreated", payload: {} });
    expect(useAppStore.getState().lastArtifactEvent).toBe(1);
    expect(useAppStore.getState().dashboard?.artifacts_total).toBe(3);

    handleEvent({
      event_id: 123,
      event_type: "HitlRequested",
      payload: {
        token: "8F2A",
        run_id: "rr1",
        workspace_id: "ws",
        checkpoint_id: "h1",
        interaction_type: "BINARY_GATE",
      },
    });
    expect(useAppStore.getState().lastRecipeEvent).toBe(1);

    handleEvent({
      event_id: 124,
      event_type: "RunStreamChunk",
      payload: {
        run_id: "r1",
        task_id: "t1",
        chunk_type: "event",
      },
    });
    expect(useAppStore.getState().lastRunChunk).toMatchObject({
      run_id: "r1",
      chunk_type: "event",
    });

    handleEvent({
      event_id: 125,
      event_type: "PermissionRequested",
      payload: {
        request_id: "req-1",
        task_id: "t1",
        workspace_id: "ws",
        action: "bash",
        resources: ["rm -rf build"],
      },
    });
    expect(useAppStore.getState().lastPermissionEvent).toMatchObject({
      request_id: "req-1",
      action: "bash",
      resources: ["rm -rf build"],
    });

    handleEvent({
      event_id: 13,
      event_type: "ScheduleUpdated",
      payload: { schedule_id: "s1", workspace_id: "ws" },
    });
    expect(useAppStore.getState().lastScheduleEvent?.schedule_id).toBe("s1");

    handleEvent({
      event_id: 14,
      event_type: "BudgetAlert",
      payload: { level: "hard", message: "cap", daily_spent: 5, daily_limit: 5 },
    });
    expect(useAppStore.getState().budgetAlert).toMatchObject({ level: "hard" });

    handleEvent({ event_id: 15, event_type: "UnknownEvent", payload: {} });
  });

  it("refreshes on resync and counts proposal events", async () => {
    const { handleEvent } = await import("./app-store");
    const get = apiGet as Mock;
    get.mockResolvedValue([]);
    handleEvent({ event_id: 30, event_type: "ResyncRequired", payload: {} });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(get).toHaveBeenCalled();

    useAppStore.setState({ lastProposalEvent: 0 });
    handleEvent({ event_id: 31, event_type: "ProposalCreated", payload: {} });
    handleEvent({ event_id: 32, event_type: "ProposalResolved", payload: {} });
    expect(useAppStore.getState().lastProposalEvent).toBe(2);
  });

  it("applies defaults for partial realtime payloads", async () => {
    const { handleEvent } = await import("./app-store");
    useAppStore.setState({
      dashboard: null,
      lastScheduleEvent: { schedule_id: "keep", workspace_id: "ws" },
    });

    handleEvent({
      event_id: 50,
      event_type: "PermissionRequested",
      payload: { request_id: "req-2", workspace_id: "ws" },
    });
    expect(useAppStore.getState().lastPermissionEvent).toMatchObject({
      request_id: "req-2",
      action: "permission",
      resources: [],
    });

    handleEvent({ event_id: 51, event_type: "RunFinished", payload: {} });
    handleEvent({
      event_id: 52,
      event_type: "ArtifactCreated",
      payload: { artifact: { id: "a1" } },
    });
    expect(useAppStore.getState().lastArtifact?.id).toBe("a1");
    expect(useAppStore.getState().dashboard).toBeNull();

    handleEvent({
      event_id: 53,
      event_type: "GoogleConnectionUpdated",
      payload: { workspace_id: "ws" },
    });
    expect(useAppStore.getState().lastGoogleEvent?.connected).toBe(false);

    handleEvent({ event_id: 54, event_type: "EngineUpdateProgress", payload: {} });
    expect(useAppStore.getState().lastEngineUpdate?.phase).toBe("running");

    handleEvent({ event_id: 55, event_type: "ScheduleUpdated", payload: {} });
    expect(useAppStore.getState().lastScheduleEvent?.schedule_id).toBe("keep");

    handleEvent({ event_id: 56, event_type: "BudgetAlert", payload: {} });
    expect(useAppStore.getState().budgetAlert).toMatchObject({
      level: "soft",
      daily_spent: 0,
      daily_limit: 0,
    });
  });

  it("applies google and engine update events", async () => {
    const { handleEvent } = await import("./app-store");
    handleEvent({
      event_id: 20,
      event_type: "GoogleConnectionUpdated",
      payload: { workspace_id: "ws", connected: true, email: "a@b.c" },
    });
    expect(useAppStore.getState().lastGoogleEvent).toMatchObject({
      workspace_id: "ws",
      connected: true,
    });

    handleEvent({
      event_id: 21,
      event_type: "EngineUpdateProgress",
      payload: { phase: "done", message: "ok", ok: true },
    });
    expect(useAppStore.getState().lastEngineUpdate).toMatchObject({
      phase: "done",
      ok: true,
    });
  });
});

class FakeWebSocket {
  static OPEN = 1;
  static CONNECTING = 0;
  static CLOSED = 3;
  static last: FakeWebSocket | null = null;
  readyState = FakeWebSocket.CONNECTING;
  onopen: (() => void) | null = null;
  onmessage: ((message: { data: string }) => void) | null = null;
  onerror: (() => void) | null = null;
  onclose: (() => void) | null = null;
  url: string;

  constructor(url: string) {
    this.url = url;
    FakeWebSocket.last = this;
  }

  close() {
    this.readyState = FakeWebSocket.CLOSED;
  }
}

describe("boot and sockets", () => {
  it("boots, refreshes and handles websocket messages", async () => {
    vi.stubGlobal("WebSocket", FakeWebSocket);
    (websocketUrl as Mock).mockResolvedValue("ws://127.0.0.1:1/ws?token=x");
    (apiGet as Mock).mockImplementation(async (path: string) => {
      if (path.includes("daemon")) return daemon;
      return [];
    });

    await useAppStore.getState().boot();
    expect(useAppStore.getState().booted).toBe(true);
    expect(useAppStore.getState().loading).toBe(false);

    const socket = FakeWebSocket.last;
    expect(socket?.url).toContain("/ws");
    socket?.onopen?.();
    expect(useAppStore.getState().connected).toBe(true);

    socket?.onmessage?.({
      data: JSON.stringify({
        event_id: 99,
        event_type: "DaemonStatusChanged",
        payload: { ...daemon, state: "online" },
      }),
    });
    expect(useAppStore.getState().daemon?.state).toBe("online");

    socket?.onmessage?.({ data: "not json" });
    socket?.onerror?.();
    expect(socket?.readyState).toBe(FakeWebSocket.CLOSED);
  });


  it("handles boot errors, empty workspaces and daemon-less restart", async () => {
    (apiGet as Mock).mockRejectedValue(new Error("offline"));
    await useAppStore.getState().boot();
    expect(useAppStore.getState().booted).toBe(false);
    expect(useAppStore.getState().error).toContain("offline");

    (apiGet as Mock).mockImplementation(async (path: string) => {
      if (path.includes("daemon")) return daemon;
      return [];
    });
    await useAppStore.getState().refresh();
    expect(useAppStore.getState().dashboard).toBeNull();

    useAppStore.setState({ daemon: null });
    (apiPost as Mock).mockResolvedValue({ ok: true });
    await useAppStore.getState().restartDaemon();
    expect(useAppStore.getState().daemon).toBeNull();
  });

  it("refreshes the dashboard for the active workspace", async () => {
    (apiGet as Mock).mockImplementation(async (path: string) => {
      if (path.includes("daemon")) return daemon;
      if (path.includes("workspaces")) return [{ id: "ws-1" }];
      if (path.includes("dashboard")) return { runs_total: 7 };
      return null;
    });
    useAppStore.setState({ activeWorkspaceId: "ws-1" });
    await useAppStore.getState().refresh();
    expect(useAppStore.getState().dashboard?.runs_total).toBe(7);
  });

  it("ignores incomplete realtime payloads", async () => {
    const { handleEvent } = await import("./app-store");
    useAppStore.setState({
      lastTaskEvent: null,
      lastRunChunk: null,
      lastPermissionEvent: null,
      lastFsEvent: null,
    });
    handleEvent({ event_id: 40, event_type: "TaskUpdated", payload: { task_id: "t1" } });
    handleEvent({ event_id: 41, event_type: "RunStreamChunk", payload: {} });
    handleEvent({ event_id: 42, event_type: "PermissionRequested", payload: {} });
    handleEvent({ event_id: 43, event_type: "FsSyncTriggered", payload: {} });
    expect(useAppStore.getState().lastTaskEvent).toBeNull();
    expect(useAppStore.getState().lastRunChunk).toBeNull();
    expect(useAppStore.getState().lastPermissionEvent).toBeNull();
    expect(useAppStore.getState().lastFsEvent).toMatchObject({
      entity: "file",
      path: "",
      workspace_id: "",
    });
  });
  it("falls back when the saved active workspace no longer exists", async () => {
    (apiGet as Mock).mockImplementation(async (path: string) => {
      if (path.includes("daemon")) return daemon;
      if (path.includes("workspaces")) return [{ id: "ws-1" }];
      if (path.includes("dashboard")) return { runs_total: 2 };
      return null;
    });
    useAppStore.setState({ activeWorkspaceId: "gone" });
    await useAppStore.getState().refresh();
    expect(useAppStore.getState().activeWorkspaceId).toBe("ws-1");
  });

  it("reconnects after close and ignores duplicate timers", async () => {
    vi.useFakeTimers();
    vi.stubGlobal("WebSocket", FakeWebSocket);
    (websocketUrl as Mock).mockResolvedValue("ws://127.0.0.1:1/ws?token=x");
    (apiGet as Mock).mockResolvedValue([]);
    await useAppStore.getState().boot();
    const first = FakeWebSocket.last;
    expect(first).not.toBeNull();
    first!.readyState = FakeWebSocket.OPEN;
    await useAppStore.getState().boot();
    expect(FakeWebSocket.last).toBe(first);

    first!.readyState = FakeWebSocket.CLOSED;
    first!.onclose?.();
    first!.onclose?.();
    expect(useAppStore.getState().connected).toBe(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(FakeWebSocket.last).not.toBe(first);
    vi.useRealTimers();
  });

  it("activates a workspace and loads its dashboard", async () => {
    const workspace = {
      id: "ws-9",
      name: "Nine",
      root_path: "/tmp/nine",
      color_hex: "#fff",
      default_router_policy: "COST_BENEFIT",
      timezone: "UTC",
      is_active: true,
      created_at: "",
      last_opened_at: "",
    };
    (apiPost as Mock).mockResolvedValue(workspace);
    (apiGet as Mock).mockResolvedValue({ runs_total: 4 });
    useAppStore.setState({ workspaces: [workspace] });

    await useAppStore.getState().selectWorkspace("ws-9");
    expect(useAppStore.getState().activeWorkspaceId).toBe("ws-9");
    expect(useAppStore.getState().dashboard?.runs_total).toBe(4);
  });
});
