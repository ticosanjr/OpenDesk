import { describe, expect, it } from "vitest";
import type { AgentGoal, AgentProfileRow, GovernanceAgent } from "./types";
import {
  budgetPercent,
  delegationTone,
  formatUsd,
  goalProgress,
  goalTone,
  orgChildren,
  orgRoots,
} from "./governance";

function row(agent: string, manager?: string, budget = 100): AgentProfileRow {
  return {
    profile: {
      workspace_id: "ws",
      agent_id: agent,
      manager_id: manager ?? null,
      monthly_budget_usd: budget,
      concurrency_quota: 5,
      heartbeat_enabled: false,
      heartbeat_minutes: 60,
      updated_at: "2026-01-01T00:00:00Z",
    },
    monthly_spend_usd: 40,
  };
}

const agents: GovernanceAgent[] = [
  { id: "lead", disabled: false, scope: "workspace" },
  { id: "build", disabled: false, scope: "workspace" },
  { id: "qa", disabled: false, scope: "workspace" },
];

describe("governance helpers", () => {
  it("builds roots and children, ignoring unknown managers", () => {
    const rows = [row("lead"), row("build", "lead"), row("qa", "missing")];
    expect(orgRoots(agents, rows).map((agent) => agent.id)).toEqual([
      "lead",
      "qa",
    ]);
    expect(orgChildren(agents, rows, "lead").map((agent) => agent.id)).toEqual([
      "build",
    ]);
  });

  it("calculates budget and goal progress", () => {
    expect(budgetPercent(row("lead"))).toBe(40);
    expect(budgetPercent(row("lead", undefined, 0))).toBeNull();
    const goal: AgentGoal = {
      id: "g",
      workspace_id: "ws",
      agent_id: "lead",
      title: "Meta",
      current_value: 3,
      target_value: 4,
      status: "ACTIVE",
      created_at: "2026-01-01T00:00:00Z",
      updated_at: "2026-01-01T00:00:00Z",
    };
    expect(goalProgress(goal)).toBe(75);
    expect(goalProgress({ ...goal, target_value: 0, status: "DONE" })).toBe(100);
    expect(goalProgress({ ...goal, target_value: 0 })).toBe(0);
  });

  it("labels tones and currency", () => {
    expect(delegationTone("PENDING")).toBe("yellow");
    expect(delegationTone("REJECTED")).toBe("red");
    expect(goalTone("DONE")).toBe("green");
    expect(goalTone("PAUSED")).toBe("yellow");
    expect(formatUsd(12.5)).toBe("US$ 12.50");
    expect(formatUsd(null)).toBe("—");
  });
});
