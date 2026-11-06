export interface DaemonStatus {
  online: boolean;
  port: number;
  active_jobs: number;
  state: string;
  message?: string | null;
  version?: string | null;
  mode?: string;
  binary_path?: string | null;
}

export interface OpenCodeBinaryInfo {
  path: string;
  version: string;
  method?: string | null;
  candidates: string[];
}

export interface EngineStatus {
  daemon: DaemonStatus;
  configured_binary?: string | null;
  resolved?: OpenCodeBinaryInfo | null;
  resolve_error?: string | null;
  candidates: string[];
  app_version: string;
}

export interface DetectResult {
  ok: boolean;
  binary?: OpenCodeBinaryInfo | null;
  error?: string | null;
  candidates: string[];
}

export interface OpenCodeRelease {
  version: string;
  tag: string;
  name: string;
  notes?: string | null;
  url: string;
  published_at?: string | null;
}

export interface OpenCodeUpdateCheck {
  current?: string | null;
  latest?: string | null;
  update_available: boolean;
  release?: OpenCodeRelease | null;
  checked_at: string;
  error?: string | null;
}

export interface EngineUpdateResult {
  ok: boolean;
  version?: string | null;
  output: string;
  daemon: DaemonStatus;
}

export interface UpdateAsset {
  name: string;
  size: number;
  sha256: string;
}

export interface LatestManifest {
  version: string;
  tag: string;
  repo: string;
  platform: string;
  published_at: string;
  notes: string;
  assets: UpdateAsset[];
}

export interface UpdateInfo {
  current: string;
  latest?: LatestManifest | null;
  update_available: boolean;
  token_configured: boolean;
  installer: string;
  checked_at: string;
  error?: string | null;
}

export interface DownloadResult {
  ok: boolean;
  action: string;
  path?: string | null;
  message: string;
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
  job_kind?: string | null;
  output_dir?: string | null;
  scheduled_for?: string | null;
  enqueued_at: string;
  started_at?: string | null;
  finished_at?: string | null;
  retry_count?: number;
  max_retries?: number;
  error_message?: string | null;
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

export interface Task {
  id: string;
  workspace_id: string;
  title: string;
  description: string;
  column_stage: string;
  phase_status?: string | null;
  priority: number;
  execution_mode: string;
  assigned_agent?: string | null;
  preferred_model?: string | null;
  effort_level: string;
  plan_markdown?: string | null;
  output_dir_path?: string | null;
  notify_email?: string | null;
  export_to_gdocs: boolean;
  tags_json: string;
  position_index: number;
  created_at: string;
  updated_at: string;
}

export interface DispatchDecision {
  phase: string;
  selected_agent: string;
  selected_model: string;
  effort: string;
  output_directory: string;
  rationale: string;
  requires_plan_approval: boolean;
  audit_after_build: boolean;
  policy_applied: string;
  rule_name?: string | null;
}

export interface TaskRun {
  id: string;
  queue_job_id?: string | null;
  workspace_id: string;
  task_id?: string | null;
  parent_run_id?: string | null;
  phase?: string | null;
  agent_name: string;
  execution_mode: string;
  provider_id?: string | null;
  model_id: string;
  effort_level: string;
  status: string;
  tokens_input: number;
  tokens_output: number;
  tokens_reasoning: number;
  tokens_cache_read: number;
  tokens_cache_write: number;
  estimated_cost_usd: number;
  duration_ms: number;
  log_path?: string | null;
  summary_response?: string | null;
  started_at: string;
  completed_at?: string | null;
  error_code?: string | null;
  error_message?: string | null;
  error_hint?: string | null;
  task_title?: string | null;
}

export interface TaskDiagnostics {
  task: Task;
  jobs: QueueJob[];
  runs: TaskRun[];
  engine: DaemonStatus;
  error: {
    code: string;
    message: string;
    hint: string;
    source: "run" | "queue";
    run_id?: string;
    job_id?: string;
  } | null;
  latest_log_tail?: string | null;
  generated_at: string;
}

export interface Artifact {
  id: string;
  workspace_id: string;
  task_id?: string | null;
  run_id: string;
  file_path: string;
  absolute_path: string;
  file_type: string;
  file_size_bytes: number;
  content_sha256?: string | null;
  action_type: string;
  created_by_agent: string;
  creation_reason: string;
  diff_patch?: string | null;
  google_doc_url?: string | null;
  google_drive_url?: string | null;
  emailed_to?: string | null;
  created_at: string;
}

export interface BudgetStatus {
  daily_spent: number;
  monthly_spent: number;
  five_hour_tokens: number;
  daily_limit: number;
  monthly_limit: number;
  five_hour_limit: number;
  daily_pct: number;
  monthly_pct: number;
  soft_alert: boolean;
  hard_stop: boolean;
  hard_stop_enabled: boolean;
}

export interface BudgetSettings {
  daily_limit_usd: number;
  monthly_limit_usd: number;
  soft_alert_threshold_pct: number;
  hard_stop_enabled: boolean;
  opencode_go_5h_token_limit: number;
  updated_at: string;
}

export interface RouterRule {
  id: string;
  workspace_id?: string | null;
  name: string;
  priority_order: number;
  condition_json: string;
  target_mode: string;
  target_agent?: string | null;
  target_model: string;
  target_effort: string;
  is_enabled: boolean;
}

export interface QueueStatus {
  paused: boolean;
  pause_reason?: string | null;
  active_jobs: number;
  concurrency: number;
  running: number;
  waiting: number;
}

export interface Schedule {
  id: string;
  workspace_id: string;
  name: string;
  description?: string | null;
  cron_expression: string;
  timezone: string;
  agent_name?: string | null;
  command_or_prompt: string;
  router_policy: string;
  output_folder_template: string;
  notify_email_recipients?: string | null;
  sync_to_google_docs: boolean;
  sync_to_google_calendar: boolean;
  is_active: boolean;
  last_run_at?: string | null;
  last_run_status?: string | null;
  next_run_at?: string | null;
  created_at: string;
}

export interface SmtpSettings {
  workspace_id: string;
  host?: string | null;
  port: number;
  username?: string | null;
  from_email?: string | null;
  use_tls: boolean;
  password_set: boolean;
  configured: boolean;
}

export interface Delivery {
  id: string;
  workspace_id: string;
  artifact_id?: string | null;
  channel: string;
  idempotency_key: string;
  status: string;
  attempts: number;
  last_error?: string | null;
  external_id?: string | null;
  external_url?: string | null;
  created_at: string;
  updated_at: string;
}

export interface GoogleStatus {
  connected: boolean;
  email?: string | null;
  scopes?: string | null;
  callback_url: string;
  client_id_set: boolean;
  client_secret_set: boolean;
}

export interface GoogleConfig {
  client_id_set: boolean;
  client_secret_set: boolean;
  scopes: string;
  callback_url: string;
}

export interface AboutInfo {
  app: {
    name: string;
    version: string;
    phase: string;
    api: string;
  };
  runtime: {
    tauri: string;
    rustc?: string | null;
    os: string;
    arch: string;
    kernel?: string | null;
  };
  engine: {
    daemon: DaemonStatus;
    configured_binary?: string | null;
    resolved?: OpenCodeBinaryInfo | null;
    installer: string;
  };
  database: {
    path: string;
    size_bytes: number;
    schema_version?: number | null;
    workspaces: number;
  };
  paths: {
    data_dir: string;
    config_dir: string;
    log_dir: string;
    db_path: string;
    backups_dir: string;
    run_logs_dir: string;
  };
  updates: {
    current: string;
    installer: string;
    token_configured: boolean;
  };
  logs: Array<{
    name?: string | null;
    path: string;
    size_bytes: number;
    modified?: string | null;
  }>;
  started_at: string;
  api_port: number;
}
