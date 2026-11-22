export interface AbForm {
  title: string;
  prompt: string;
  agentA: string;
  agentB: string;
  modelA: string;
  modelB: string;
}

export function worktreeTone(status: string) {
  if (status === "MERGED") return "green" as const;
  if (status === "REMOVED") return "red" as const;
  return "blue" as const;
}

export function abTone(status: string) {
  if (status === "DECIDED") return "green" as const;
  if (status === "CANCELLED") return "red" as const;
  return "blue" as const;
}
