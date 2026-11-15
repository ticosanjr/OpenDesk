export interface FailureCode {
  code: string;
  count: number;
}

export interface AgentMetrics {
  agent_id: string;
  runs: number;
  success_rate: number;
  cost_usd: number;
  p95_duration_ms: number;
}

export interface DailyMetrics {
  date: string;
  runs: number;
  failed: number;
  cost_usd: number;
}

export interface Metrics {
  days: number;
  runs_total: number;
  runs_success: number;
  runs_failed: number;
  runs_cancelled: number;
  chat_messages: number;
  success_rate: number;
  cost_usd: number;
  tokens: number;
  p50_duration_ms: number;
  p95_duration_ms: number;
  avg_duration_ms: number;
  queue_samples: number;
  queue_p50_wait_ms: number;
  queue_p95_wait_ms: number;
  queue_avg_wait_ms: number;
  failure_codes: FailureCode[];
  agents: AgentMetrics[];
  sources: SourceMetrics[];
  daily: DailyMetrics[];
}

export interface SourceMetrics {
  source: string;
  channel?: string | null;
  executions: number;
  messages: number;
  tokens: number;
  cost_usd: number;
  success_rate: number;
  p95_duration_ms: number;
}
