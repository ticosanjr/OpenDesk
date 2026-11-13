export type StatusTone =
  | "default"
  | "blue"
  | "green"
  | "yellow"
  | "red"
  | "purple";

export const RUN_STATUS_LABEL: Record<string, string> = {
  PENDING: "pendente",
  RUNNING: "executando",
  SUCCESS: "sucesso",
  FAILED: "falhou",
  CANCELLED: "cancelada",
};

export function runStatusLabel(status: string): string {
  return RUN_STATUS_LABEL[status] ?? status.toLowerCase();
}

export function runStatusTone(status: string): StatusTone {
  if (status === "SUCCESS") return "green";
  if (status === "FAILED") return "red";
  if (status === "RUNNING") return "blue";
  if (status === "CANCELLED" || status === "PENDING") return "yellow";
  return "default";
}

export const RECIPE_RUN_STATUS_LABEL: Record<string, string> = {
  COMPLETED: "concluída",
  FAILED: "falhou",
  REJECTED: "rejeitada",
  RUNNING: "executando",
  WAITING: "aguardando",
  WAITING_INPUT: "aguardando",
  WAITING_APPROVAL: "aguardando",
};

export function recipeRunStatusLabel(status: string): string {
  if (RECIPE_RUN_STATUS_LABEL[status]) return RECIPE_RUN_STATUS_LABEL[status];
  if (status.includes("WAITING")) return "aguardando";
  return status.toLowerCase();
}

export function recipeRunStatusTone(status: string): StatusTone {
  if (status === "COMPLETED") return "green";
  if (status === "FAILED" || status === "REJECTED") return "red";
  if (status.includes("WAITING")) return "yellow";
  if (status === "RUNNING") return "blue";
  return "default";
}
