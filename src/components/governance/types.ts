export type AgentProfileValues = {
  role: string;
  team: string;
  managerId: string;
  budget: string;
  quota: string;
  heartbeat: boolean;
  heartbeatMinutes: string;
  notes: string;
};

export type GoalValues = {
  agentId: string;
  title: string;
  description: string;
  metric: string;
  target: string;
  current: string;
  unit: string;
  dueAt: string;
  status: string;
};

export type DelegationValues = {
  fromAgent: string;
  toAgent: string;
  title: string;
  prompt: string;
};

export function emptyProfile(): AgentProfileValues {
  return {
    role: "",
    team: "",
    managerId: "",
    budget: "",
    quota: "5",
    heartbeat: false,
    heartbeatMinutes: "60",
    notes: "",
  };
}

export function emptyGoal(): GoalValues {
  return {
    agentId: "",
    title: "",
    description: "",
    metric: "",
    target: "",
    current: "0",
    unit: "",
    dueAt: "",
    status: "ACTIVE",
  };
}
