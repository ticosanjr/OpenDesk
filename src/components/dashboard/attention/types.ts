export interface PermissionRequest {
  id: string;
  request_id: string;
  task_id?: string | null;
  run_id?: string | null;
  permission_action: string;
  resources?: string | null;
  status: string;
  created_at: string;
}

export interface HitlRequest {
  token: string;
  recipe_run_id: string;
  workspace_id: string;
  checkpoint_id: string;
  interaction_type: string;
  title?: string | null;
  question_markdown?: string | null;
  context_json?: string | null;
  options_json?: string | null;
  on_timeout?: string | null;
  expires_at?: string | null;
  status: string;
  created_at: string;
}

export interface Proposal {
  id: string;
  kind: string;
  action: string;
  title: string;
  description?: string | null;
  status: string;
  requested_by?: string | null;
  apply_error?: string | null;
}

export interface Attention {
  requests: PermissionRequest[];
  missed_runs: Array<{
    id: string;
    workspace_id: string;
    schedule_id: string;
    schedule_name?: string | null;
    scheduled_for: string;
    detected_at: string;
    reason: string;
    status: string;
    run_id?: string | null;
    error_message?: string | null;
  }>;
  task_interactions: Array<{
    id: string;
    workspace_id: string;
    task_id: string;
    run_id?: string | null;
    question: string;
    context_markdown?: string | null;
    options_json: string;
    multiple: boolean;
    allow_custom: boolean;
    status: string;
    answer_json?: string | null;
    expires_at: string;
    created_at: string;
    resolved_at?: string | null;
  }>;
  hitl: HitlRequest[];
  proposals: Proposal[];
  awaiting_approval: Array<{
    id: string;
    title: string;
    phase_status?: string | null;
    plan_excerpt?: string | null;
    version?: string | null;
  }>;
  blocked: Array<{
    id: string;
    title: string;
    reason?: string | null;
    version?: string | null;
  }>;
  failed_runs: Array<{
    id: string;
    task_id?: string | null;
    title?: string | null;
    status: string;
    error?: string | null;
    version?: string | null;
  }>;
}

export const EMPTY_ATTENTION: Attention = {
  requests: [],
  missed_runs: [],
  task_interactions: [],
  hitl: [],
  proposals: [],
  awaiting_approval: [],
  blocked: [],
  failed_runs: [],
};
