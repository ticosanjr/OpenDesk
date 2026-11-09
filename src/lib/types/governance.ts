export interface GovernanceAgent {
  id: string;
  name?: string | null;
  description?: string | null;
  mode?: string | null;
  avatar?: string | null;
  color?: string | null;
  model?: string | null;
  disabled: boolean;
  scope: string;
}

export interface AgentProfile {
  workspace_id: string;
  agent_id: string;
  role?: string | null;
  team?: string | null;
  manager_id?: string | null;
  monthly_budget_usd?: number | null;
  concurrency_quota: number;
  heartbeat_enabled: boolean;
  heartbeat_minutes: number;
  last_heartbeat_at?: string | null;
  next_heartbeat_at?: string | null;
  notes?: string | null;
  updated_at: string;
}

export interface AgentProfileRow {
  profile: AgentProfile;
  monthly_spend_usd: number;
  budget_remaining_usd?: number | null;
  budget_percent?: number | null;
}

export interface AgentGoal {
  id: string;
  workspace_id: string;
  agent_id: string;
  title: string;
  description?: string | null;
  metric?: string | null;
  target_value?: number | null;
  current_value: number;
  unit?: string | null;
  due_at?: string | null;
  status: "ACTIVE" | "DONE" | "PAUSED" | "CANCELLED" | string;
  created_at: string;
  updated_at: string;
}

export interface AgentPerformance {
  agent_id: string;
  runs: number;
  success: number;
  failed: number;
  cancelled: number;
  cost_usd: number;
  avg_duration_ms: number;
  tokens: number;
  success_rate: number;
}

export interface AgentDelegation {
  id: string;
  workspace_id: string;
  from_agent: string;
  to_agent: string;
  title: string;
  prompt: string;
  status: "PENDING" | "APPROVED" | "REJECTED" | "FAILED" | string;
  task_id?: string | null;
  requested_by?: string | null;
  requested_at: string;
  resolved_at?: string | null;
  resolved_by?: string | null;
  error?: string | null;
}

export interface GovernanceOverview {
  agents: GovernanceAgent[];
  profiles: AgentProfileRow[];
  goals: AgentGoal[];
  performance: AgentPerformance[];
  delegations: AgentDelegation[];
}
