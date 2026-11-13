export const STATUS_LABEL: Record<string, string> = {
  TODO: "A fazer",
  IN_PROGRESS: "Em execução",
  IN_REVIEW: "Em revisão",
  BLOCKED: "Bloqueada",
  DONE: "Concluída",
  CANCELLED: "Cancelada",
};

export const PRIORITY_LABEL: Record<number, string> = {
  1: "baixa",
  2: "normal",
  3: "alta",
  4: "crítica",
};

export const PRIORITY_TONE: Record<number, "default" | "blue" | "yellow" | "red"> = {
  1: "default",
  2: "blue",
  3: "yellow",
  4: "red",
};

export const ORIGIN_LABEL: Record<string, string> = {
  MANUAL: "trabalho",
  RECIPE: "receita",
  IO_BUS: "io bus",
  CAPTURE: "captura",
  ROUTINE: "rotina",
  DELEGATION: "delegação",
};

export const EFFORT_TONE: Record<string, "default" | "blue" | "purple"> = {
  low: "default",
  medium: "blue",
  high: "purple",
};

export { RUN_STATUS_LABEL, runStatusLabel } from "./status";
