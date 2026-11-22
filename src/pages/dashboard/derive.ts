import type { Artifact, BudgetStatus, DashboardStats, TaskRun } from "@/lib/types";
import type { Activity } from "@/components/dashboard/ActivityFeed";

export interface ModelUsage {
  model: string;
  calls: number;
  tokens: number;
  cost: number;
  duration: number;
}

export interface DashboardMetrics {
  runsTotal: number;
  successRate: number;
  cacheHit: number;
  spent: number;
  dailyLimit: number;
  totalSpent: number;
}

export function dashboardMetrics(
  dashboard: DashboardStats | null,
  budget: BudgetStatus | null,
  fallbackDailyLimit: number,
): DashboardMetrics {
  const runsTotal = dashboard?.runs_total ?? 0;
  const successRate =
    runsTotal > 0 ? ((dashboard?.runs_success ?? 0) / runsTotal) * 100 : 0;
  const cacheBase =
    (dashboard?.tokens_cache_read ?? 0) + (dashboard?.tokens_input ?? 0);
  const cacheHit =
    cacheBase > 0 ? ((dashboard?.tokens_cache_read ?? 0) / cacheBase) * 100 : 0;
  const spent = dashboard?.daily_cost_usd ?? 0;
  const dailyLimit = budget?.daily_limit ?? fallbackDailyLimit;
  const totalSpent = dashboard?.estimated_cost_usd ?? 0;
  return { runsTotal, successRate, cacheHit, spent, dailyLimit, totalSpent };
}

export function buildModelUsage(recentRuns: TaskRun[]): ModelUsage[] {
  return Object.values(
    recentRuns.reduce<Record<string, ModelUsage>>(
      (accumulator, run) => {
        const key = run.model_id || "unknown";
        const entry = accumulator[key] ?? {
          model: key,
          calls: 0,
          tokens: 0,
          cost: 0,
          duration: 0,
        };
        entry.calls += 1;
        entry.tokens += run.tokens_input + run.tokens_output;
        entry.cost += run.estimated_cost_usd;
        entry.duration += run.duration_ms;
        accumulator[key] = entry;
        return accumulator;
      },
      {},
    ),
  ).sort((a, b) => b.cost - a.cost);
}

export function buildActivities(
  recentRuns: TaskRun[],
  recentArtifacts: Artifact[],
): Activity[] {
  return [
    ...recentRuns.map((run) => ({
      kind: "run" as const,
      at: run.completed_at ?? run.started_at,
      run,
    })),
    ...recentArtifacts.map((artifact) => ({
      kind: "artifact" as const,
      at: artifact.created_at,
      artifact,
    })),
  ]
    .sort((left, right) => right.at.localeCompare(left.at))
    .slice(0, 8);
}
