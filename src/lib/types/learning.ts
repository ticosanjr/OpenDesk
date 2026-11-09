export interface LearningSettings {
  enabled: boolean;
  capture_successes: boolean;
  auto_propose: boolean;
  min_occurrences: number;
}

export interface LearningLesson {
  id: string;
  workspace_id: string;
  run_id?: string | null;
  task_id?: string | null;
  agent_name?: string | null;
  model_id?: string | null;
  category: string;
  severity: string;
  title: string;
  detail: string;
  evidence_json: string;
  fingerprint: string;
  occurrences: number;
  status: "OPEN" | "DISMISSED" | "PROMOTED" | string;
  proposal_id?: string | null;
  remembered_at?: string | null;
  last_seen_at: string;
  created_at: string;
  updated_at: string;
}

export interface LearningStats {
  total: number;
  open: number;
  promoted: number;
  dismissed: number;
  runs_analyzed: number;
  failed_runs: number;
  success_rate: number;
}

export interface LearningProposal {
  id: string;
  kind: string;
  action: string;
  title: string;
  description?: string | null;
  status: string;
  requested_by?: string | null;
  apply_error?: string | null;
  payload_json?: string | null;
  created_at: string;
}

export interface LearningOverview {
  settings: LearningSettings;
  stats: LearningStats;
  lessons: LearningLesson[];
  proposals: LearningProposal[];
}

export interface LearningScanResult {
  analyzed: number;
  lessons: number;
}
