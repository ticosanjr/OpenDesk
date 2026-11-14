import type { DaemonStatus } from "./engine";

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
  daily_cost_usd: number;
  queue_running: number;
  queue_waiting: number;
  artifacts_total: number;
  active_routines: number;
  enabled_recipes: number;
}

export interface QueueJob {
  id: string;
  task_id?: string | null;
  session_id?: string | null;
  title?: string | null;
  status: string;
  priority: number;
  resolved_mode: string;
  resolved_agent: string;
  resolved_model: string;
  resolved_effort: string;
  job_kind?: string | null;
  task_origin?: string | null;
  output_dir?: string | null;
  scheduled_for?: string | null;
  enqueued_at: string;
  started_at?: string | null;
  finished_at?: string | null;
  retry_count?: number;
  max_retries?: number;
  concurrency_group?: string | null;
  lease_expires_at?: string | null;
  last_heartbeat_at?: string | null;
  error_message?: string | null;
}

export interface ApiEnvelope<T> {
  data: T;
}

export type WsEvent = {
  event_id: number;
  event_type: string;
  payload: unknown;
};

export interface Task {
  id: string;
  workspace_id: string;
  title: string;
  description: string;
  status: string;
  phase?: string | null;
  phase_status?: string | null;
  priority: number;
  execution_mode: string;
  assigned_agent?: string | null;
  preferred_model?: string | null;
  agent_override?: boolean;
  model_override?: boolean;
  effort_level: string;
  care_level: "quick" | "standard" | "careful";
  plan_markdown?: string | null;
  output_dir_path?: string | null;
  notify_email?: string | null;
  export_to_gdocs: boolean;
  tags_json: string;
  position_index: number;
  last_run_id?: string | null;
  last_run_status?: string | null;
  forked_from_task_id?: string | null;
  last_run_at?: string | null;
  blocked_reason?: string | null;
  cancelled_at?: string | null;
  origin:
    | "MANUAL"
    | "RECIPE"
    | "IO_BUS"
    | "CAPTURE"
    | "ROUTINE"
    | "DELEGATION"
    | string;
  parent_task_id?: string | null;
  dedupe_key?: string | null;
  deduplicated?: boolean;
  held: boolean;
  active_job_status?: string | null;
  active_run_status?: string | null;
  workflow_state?: string | null;
  workflow_reason?: string | null;
  created_at: string;
  updated_at: string;
}

export interface TaskWatcher {
  id: string;
  workspace_id: string;
  task_id: string;
  channel: "telegram" | "signal" | string;
  recipient?: string | null;
  created_at: string;
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
  origin: string;
}

export interface MissedRun {
  id: string;
  workspace_id: string;
  schedule_id: string;
  schedule_name?: string | null;
  scheduled_for: string;
  detected_at: string;
  reason: string;
  status: "PENDING" | "QUEUED" | "IGNORED" | "FAILED" | "CANCELLED" | string;
  run_id?: string | null;
  error_message?: string | null;
}

export interface TaskInteraction {
  id: string;
  workspace_id: string;
  task_id: string;
  run_id?: string | null;
  question: string;
  context_markdown?: string | null;
  options_json: string;
  multiple: boolean;
  allow_custom: boolean;
  status: "PENDING" | "RESOLVED" | "EXPIRED" | "CANCELLED" | string;
  answer_json?: string | null;
  expires_at: string;
  created_at: string;
  resolved_at?: string | null;
}

export interface TaskRun {
  id: string;
  queue_job_id?: string | null;
  workspace_id: string;
  task_id?: string | null;
  schedule_id?: string | null;
  attempt_number: number;
  session_id?: string | null;
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
  pricing_snapshot_json?: string | null;
  duration_ms: number;
  log_path?: string | null;
  raw_stdout_log?: string | null;
  summary_response?: string | null;
  started_at: string;
  completed_at?: string | null;
  error_code?: string | null;
  error_message?: string | null;
  error_hint?: string | null;
  task_title?: string | null;
  task_origin?: string | null;
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
  pinned?: boolean;
  created_at: string;
}

export interface TaskMessage {
  id: string;
  task_id: string;
  run_id?: string | null;
  role: string;
  kind: string;
  content: string;
  metadata_json?: string | null;
  created_at: string;
}
