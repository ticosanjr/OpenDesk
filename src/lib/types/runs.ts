import type { TaskRun } from "./core";

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

export interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
  size: number;
  modified?: string | null;
}

export interface FileContent {
  path: string;
  content: string;
  truncated: boolean;
  binary: boolean;
  mime: string;
  size: number;
}

export interface TimelineTool {
  id: string;
  name: string;
  summary: string;
  input: string;
  output: string;
  status: string;
  started_at?: string | null;
  ended_at?: string | null;
  duration_ms: number;
  output_truncated: boolean;
}

export interface TimelineSection {
  started_at?: string | null;
  ended_at?: string | null;
  duration_ms: number;
  text: string;
  tools: TimelineTool[];
  truncated: boolean;
}

export interface TimelineStep {
  index: number;
  attempt_number?: number | null;
  agent?: string | null;
  model?: string | null;
  started_at?: string | null;
  ended_at?: string | null;
  duration_ms: number;
  tokens_input: number;
  tokens_output: number;
  tokens_reasoning: number;
  cost_usd: number;
  finish?: string | null;
}

export interface RunAttempt {
  id: string;
  attempt_number: number;
  status: string;
  phase?: string | null;
  started_at: string;
  completed_at?: string | null;
  duration_ms: number;
  tokens_input: number;
  tokens_output: number;
  cost_usd: number;
  error_message?: string | null;
}

export interface RunTimeline {
  run?: TaskRun | null;
  root_run_id?: string | null;
  attempts?: RunAttempt[];
  status: string;
  started_at?: string | null;
  completed_at?: string | null;
  duration_ms: number;
  events_count: number;
  steps: TimelineStep[];
  thinking: TimelineSection;
  executed: TimelineSection;
  result: TimelineSection;
  log_missing: boolean;
  workspace_root?: string | null;
  output_directory?: string | null;
  output_directory_relative?: string | null;
  artifacts: TimelineArtifact[];
  inputs?: TimelineInput[];
  events: TimelineEvent[];
}

export interface TimelineInput {
  name: string;
  path: string;
  absolute_path: string;
  size?: number | null;
  source?: string | null;
  trigger_type?: string | null;
}

export interface TimelineArtifact {
  id: string;
  file_path: string;
  absolute_path: string;
  action_type: string;
  created_at: string;
}

export interface TimelineEvent {
  kind: "thinking" | "tool" | "comment" | "step" | "event" | string;
  role: string;
  run_id?: string | null;
  attempt_number?: number | null;
  text: string;
  event_type?: string;
  data?: string;
  tool?: TimelineTool | null;
  step_index?: number | null;
  started_at?: string | null;
  ended_at?: string | null;
  duration_ms: number;
}

export interface SearchHit {
  id: string;
  kind: string;
  title: string;
  snippet: string;
  updated_at?: string | null;
  link: string;
}
