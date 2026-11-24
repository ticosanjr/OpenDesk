import type {
  Artifact,
  BudgetStatus,
  DaemonStatus,
  DashboardStats,
  QueueJob,
  QueueStatus,
  Workspace,
} from "@/lib/types";

export interface LiveRevisions {
  queue: number;
  tasks: number;
  runs: number;
  artifacts: number;
  attention: number;
  schedules: number;
  recipes: number;
  memory: number;
  governance: number;
  studio: number;
}

export interface CreateWorkspaceInput {
  name: string;
  root_path: string;
  description?: string;
  color_hex?: string;
  default_router_policy?: string;
  timezone?: string;
}

export interface AppStore {
  booted: boolean;
  loading: boolean;
  error: string | null;
  connected: boolean;
  daemon: DaemonStatus | null;
  workspaces: Workspace[];
  activeWorkspaceId: string | null;
  dashboard: DashboardStats | null;
  budget: BudgetStatus | null;
  queue: QueueJob[];
  queueStatus: QueueStatus | null;
  attentionCount: number;
  revisions: LiveRevisions;
  wsConnected: boolean;
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
    workspace_id?: string;
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
  lastChatEvent: {
    thread_id: string;
    workspace_id: string;
    seq: number;
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
  refreshLiveState: (workspaceId?: string | null) => Promise<void>;
  setAttentionCount: (count: number) => void;
  selectWorkspace: (id: string) => Promise<void>;
  createWorkspace: (input: CreateWorkspaceInput) => Promise<Workspace>;
  restartDaemon: () => Promise<void>;
}
