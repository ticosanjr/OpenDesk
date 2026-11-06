export interface DaemonStatus {
  online: boolean;
  port: number;
  active_jobs: number;
  state: string;
  message?: string | null;
  version?: string | null;
}

export interface Workspace {
  id: string;
  name: string;
  root_path: string;
  description?: string | null;
  color_hex: string;
  default_router_policy: string;
  default_email_recipient?: string | null;
  google_drive_folder_id?: string | null;
  timezone: string;
  is_active: boolean;
  created_at: string;
  last_opened_at: string;
}

export interface DashboardStats {
  runs_total: number;
  runs_success: number;
  runs_failed: number;
  runs_running: number;
  tokens_input: number;
  tokens_output: number;
  tokens_cache_read: number;
  tokens_cache_write: number;
  estimated_cost_usd: number;
  queue_running: number;
  queue_waiting: number;
  artifacts_total: number;
}

export interface QueueJob {
  id: string;
  task_id?: string | null;
  title?: string | null;
  status: string;
  priority: number;
  resolved_mode: string;
  resolved_agent: string;
  resolved_model: string;
  resolved_effort: string;
  enqueued_at: string;
  started_at?: string | null;
}

export interface ModelInfo {
  id: string;
  modelID?: string | null;
  providerID?: string | null;
  name?: string | null;
  variants?: { id: string }[];
  status?: string | null;
  enabled?: boolean | null;
}

export interface AgentInfo {
  id: string;
  name?: string | null;
  description?: string | null;
  mode?: string | null;
  model?: { providerID: string; id: string; variant?: string | null } | null;
  color?: string | null;
  hidden?: boolean;
  permissions?: unknown;
}

export interface ApiEnvelope<T> {
  data: T;
}

export type WsEvent = {
  event_id: number;
  event_type: string;
  payload: unknown;
};

export type StudioScope = "workspace" | "global";

export interface PermissionRule {
  action: string;
  resource: string;
  effect: string;
}

export interface AgentFile {
  id: string;
  scope: string;
  path: string;
  name?: string | null;
  avatar?: string | null;
  description?: string | null;
  mode?: string | null;
  model?: string | null;
  color?: string | null;
  hidden: boolean;
  steps?: number | null;
  disabled: boolean;
  permissions: PermissionRule[];
  system: string;
  sha256: string;
}

export interface SkillExtraFile {
  path: string;
  content: string;
}

export interface SkillFile {
  id: string;
  scope: string;
  path: string;
  name: string;
  description?: string | null;
  body: string;
  files: SkillExtraFile[];
  sha256: string;
}

export interface CommandFile {
  id: string;
  scope: string;
  path: string;
  description?: string | null;
  agent?: string | null;
  model?: string | null;
  subagent?: boolean | null;
  template: string;
  sha256: string;
}

export interface ConfigDocument {
  path: string;
  content: string;
  sha256: string;
  exists: boolean;
  parsed: unknown;
}

export interface McpServerEntry {
  name: string;
  server: Record<string, unknown>;
}

export interface McpListResponse {
  document: ConfigDocument;
  servers: McpServerEntry[];
}

export const ROUTER_POLICY_LABELS: Record<string, string> = {
  COST_BENEFIT: "Melhor custo-benefício",
  CHEAPEST: "Mais barato",
  BEST_QUALITY: "Máxima qualidade",
  MANUAL: "Manual",
  CUSTOM_RULES: "Regras customizadas",
};
