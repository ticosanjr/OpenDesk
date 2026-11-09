import type {
  AgentDelegation,
  AgentGoal,
  AgentProfileRow,
  GovernanceAgent,
} from "./types";

export function budgetPercent(row: AgentProfileRow): number | null {
  const limit = row.profile.monthly_budget_usd;
  if (limit === null || limit === undefined || limit <= 0) return null;
  return Math.min(100, Math.max(0, (row.monthly_spend_usd / limit) * 100));
}

export function goalProgress(goal: AgentGoal): number {
  const target = goal.target_value;
  if (target === null || target === undefined || target <= 0) {
    return goal.status === "DONE" ? 100 : 0;
  }
  return Math.min(100, Math.max(0, (goal.current_value / target) * 100));
}

export function delegationTone(
  status: string,
): "default" | "green" | "yellow" | "red" | "purple" {
  if (status === "PENDING") return "yellow";
  if (status === "APPROVED") return "green";
  if (status === "REJECTED") return "red";
  if (status === "FAILED") return "red";
  return "default";
}

export function goalTone(
  status: string,
): "default" | "green" | "yellow" | "purple" {
  if (status === "DONE") return "green";
  if (status === "PAUSED") return "yellow";
  if (status === "ACTIVE") return "purple";
  return "default";
}

export function profileFor(
  rows: AgentProfileRow[],
  agentId: string,
): AgentProfileRow | undefined {
  return rows.find((row) => row.profile.agent_id === agentId);
}

export function orgRoots(
  agents: GovernanceAgent[],
  rows: AgentProfileRow[],
): GovernanceAgent[] {
  const ids = new Set(agents.map((agent) => agent.id));
  return agents.filter((agent) => {
    const manager = profileFor(rows, agent.id)?.profile.manager_id;
    return !manager || !ids.has(manager) || manager === agent.id;
  });
}

export function orgChildren(
  agents: GovernanceAgent[],
  rows: AgentProfileRow[],
  managerId: string,
): GovernanceAgent[] {
  return agents.filter(
    (agent) =>
      agent.id !== managerId &&
      profileFor(rows, agent.id)?.profile.manager_id === managerId,
  );
}

export function formatUsd(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return "—";
  }
  return `US$ ${value.toFixed(2)}`;
}

export function delegationSummary(delegation: AgentDelegation): string {
  return `@${delegation.from_agent} → @${delegation.to_agent}`;
}
